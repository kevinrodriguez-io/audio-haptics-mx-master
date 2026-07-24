//! High-level engine: PCM → onsets → mapper → HID++ haptics.

use crate::audio::AudioRing;
use crate::dsp::OnsetDetector;
use crate::mapper::{HitMapper, MapperConfig};
use crate::transport::{open_best_transport_with_retry, HapticTransport, LinkKind};
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub sample_rate: f32,
    pub sensitivity: f32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000.0,
            sensitivity: 0.65,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineStatus {
    pub running: bool,
    pub link: LinkKind,
    pub sensitivity: f32,
    pub last_error: Option<String>,
    pub hits_fired: u64,
}

pub struct Engine {
    cfg: Mutex<EngineConfig>,
    ring: AudioRing,
    running: AtomicBool,
    status: Mutex<EngineStatus>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Arc<Self> {
        Arc::new(Self {
            status: Mutex::new(EngineStatus {
                running: false,
                link: LinkKind::None,
                sensitivity: cfg.sensitivity,
                last_error: None,
                hits_fired: 0,
            }),
            cfg: Mutex::new(cfg),
            ring: AudioRing::new(48_000 * 2), // ~1s stereo
            running: AtomicBool::new(false),
            worker: Mutex::new(None),
        })
    }

    pub fn push_audio(&self, interleaved_stereo: &[f32]) {
        self.ring.push_interleaved(interleaved_stereo);
    }

    pub fn set_sensitivity(&self, sensitivity: f32) {
        let s = sensitivity.clamp(0.05, 1.0);
        self.cfg.lock().sensitivity = s;
        self.status.lock().sensitivity = s;
    }

    pub fn status(&self) -> EngineStatus {
        self.status.lock().clone()
    }

    pub fn start(self: &Arc<Self>) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let transport = match open_best_transport_with_retry(5, Duration::from_millis(300)) {
            Ok(t) => t,
            Err(e) => {
                let msg = e.to_string();
                self.status.lock().last_error = Some(msg.clone());
                return Err(msg);
            }
        };

        {
            let mut st = self.status.lock();
            st.running = true;
            st.link = transport.link_kind();
            st.last_error = None;
        }
        self.running.store(true, Ordering::SeqCst);

        let engine = Arc::clone(self);
        let handle = thread::Builder::new()
            .name("music-drums-engine".into())
            .spawn(move || engine_loop(engine, transport))
            .map_err(|e| {
                self.running.store(false, Ordering::SeqCst);
                let msg = e.to_string();
                self.status.lock().last_error = Some(msg.clone());
                self.status.lock().running = false;
                msg
            })?;
        *self.worker.lock() = Some(handle);
        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker.lock().take() {
            let _ = handle.join();
        }
        let mut st = self.status.lock();
        st.running = false;
        st.link = LinkKind::None;
    }
}

fn engine_loop(engine: Arc<Engine>, mut transport: Box<dyn HapticTransport>) {
    let sample_rate = engine.cfg.lock().sample_rate;
    let mut detector = OnsetDetector::new(sample_rate, engine.cfg.lock().sensitivity);
    let mut mapper = HitMapper::new(MapperConfig {
        sensitivity: engine.cfg.lock().sensitivity,
        ..MapperConfig::default()
    });

    let mut current_intensity: u8 = 60;
    if let Err(e) = transport.set_haptic(true, current_intensity) {
        engine.status.lock().last_error = Some(e.to_string());
    }

    let mut reconnect_backoff = Duration::from_millis(250);

    while engine.running.load(Ordering::SeqCst) {
        // Apply sensitivity live.
        let sens = engine.cfg.lock().sensitivity;
        detector.set_sensitivity(sens);
        mapper.set_sensitivity(sens);

        let samples = engine.ring.drain(4096);
        if samples.is_empty() {
            thread::sleep(Duration::from_millis(4));
            continue;
        }

        let onsets = if samples.len() >= 2 {
            detector.process_interleaved_stereo(&samples)
        } else {
            detector.process_mono(&samples)
        };

        for onset in onsets {
            if let Some(hit) = mapper.map(onset) {
                if hit.intensity != current_intensity {
                    current_intensity = hit.intensity;
                    if let Err(e) = transport.set_haptic(true, current_intensity) {
                        engine.status.lock().last_error = Some(e.to_string());
                        // Attempt reconnect
                        match open_best_transport_with_retry(3, reconnect_backoff) {
                            Ok(t) => {
                                transport = t;
                                engine.status.lock().link = transport.link_kind();
                                let _ = transport.set_haptic(true, current_intensity);
                                reconnect_backoff = Duration::from_millis(250);
                            }
                            Err(err) => {
                                engine.status.lock().last_error = Some(err.to_string());
                                reconnect_backoff =
                                    (reconnect_backoff * 2).min(Duration::from_secs(3));
                            }
                        }
                        continue;
                    }
                }
                match transport.trigger(hit.pulse) {
                    Ok(()) => {
                        engine.status.lock().hits_fired += 1;
                    }
                    Err(e) => {
                        engine.status.lock().last_error = Some(e.to_string());
                        if let Ok(t) =
                            open_best_transport_with_retry(3, reconnect_backoff)
                        {
                            transport = t;
                            engine.status.lock().link = transport.link_kind();
                            let _ = transport.set_haptic(true, current_intensity);
                            let _ = transport.trigger(hit.pulse);
                        }
                    }
                }
            }
        }
    }

    let _ = transport.set_haptic(false, 0);
}

/// Manual test helper: fire a single strong pulse (Options+ must be parked).
pub fn fire_test_pulse(intensity: u8) -> Result<LinkKind, String> {
    let mut t = open_best_transport_with_retry(5, Duration::from_millis(200))
        .map_err(|e| e.to_string())?;
    let link = t.link_kind();
    t.set_haptic(true, intensity.min(100))
        .map_err(|e| e.to_string())?;
    t.trigger(crate::hidpp::HapticPulse::single(
        crate::hidpp::PulseType::Strong,
    ))
    .map_err(|e| e.to_string())?;
    Ok(link)
}
