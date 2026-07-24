//! High-level engine: PCM → onsets → mapper → HID++ haptics.

use crate::audio::AudioRing;
use crate::config::{load_active_or_default, persist_active, DrumsConfig};
use crate::dsp::OnsetDetector;
use crate::mapper::{HitMapper, MapperConfig};
use crate::transport::{open_best_transport_with_retry, HapticTransport, LinkKind};
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub sample_rate: f32,
    pub drums: DrumsConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000.0,
            drums: load_active_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineStatus {
    pub running: bool,
    pub link: LinkKind,
    pub sensitivity: f32,
    pub preset_id: String,
    pub preset_name: String,
    pub last_error: Option<String>,
    pub hits_fired: u64,
}

pub struct Engine {
    cfg: Mutex<EngineConfig>,
    config_gen: AtomicU64,
    ring: AudioRing,
    running: AtomicBool,
    status: Mutex<EngineStatus>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Arc<Self> {
        let drums = cfg.drums.clone();
        Arc::new(Self {
            status: Mutex::new(EngineStatus {
                running: false,
                link: LinkKind::None,
                sensitivity: drums.sensitivity,
                preset_id: drums.id.clone(),
                preset_name: drums.name.clone(),
                last_error: None,
                hits_fired: 0,
            }),
            cfg: Mutex::new(cfg),
            config_gen: AtomicU64::new(1),
            ring: AudioRing::new(48_000 * 2),
            running: AtomicBool::new(false),
            worker: Mutex::new(None),
        })
    }

    pub fn push_audio(&self, interleaved_stereo: &[f32]) {
        self.ring.push_interleaved(interleaved_stereo);
    }

    pub fn set_sensitivity(&self, sensitivity: f32) {
        let s = sensitivity.clamp(0.05, 1.0);
        {
            let mut cfg = self.cfg.lock();
            cfg.drums.sensitivity = s;
        }
        self.status.lock().sensitivity = s;
        // Do not bump config_gen or rewrite active.json here — that was resetting
        // the engine loop (and HID intensity) on every slider tick.
    }

    pub fn set_sample_rate(&self, sample_rate: f32) {
        let sr = sample_rate.clamp(16_000.0, 192_000.0);
        let mut cfg = self.cfg.lock();
        if (cfg.sample_rate - sr).abs() > 1.0 {
            cfg.sample_rate = sr;
            self.config_gen.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn config(&self) -> DrumsConfig {
        self.cfg.lock().drums.clone()
    }

    pub fn set_config(&self, drums: DrumsConfig) -> Result<(), String> {
        let drums = drums.sanitize();
        // Persist best-effort; still apply in-memory if disk is unavailable.
        let _ = persist_active(&drums);
        {
            let mut cfg = self.cfg.lock();
            cfg.drums = drums.clone();
        }
        let mut st = self.status.lock();
        st.sensitivity = drums.sensitivity;
        st.preset_id = drums.id.clone();
        st.preset_name = drums.name.clone();
        self.config_gen.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn status(&self) -> EngineStatus {
        self.status.lock().clone()
    }

    pub fn start(self: &Arc<Self>) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let transport = match open_best_transport_with_retry(12, Duration::from_millis(250)) {
            Ok(t) => t,
            Err(e) => {
                let msg = format!(
                    "{e} (Tip: wake the mouse, wait a second, toggle Drums mode again.)"
                );
                self.status.lock().last_error = Some(msg.clone());
                return Err(msg);
            }
        };

        {
            let drums = self.cfg.lock().drums.clone();
            let mut st = self.status.lock();
            st.running = true;
            st.link = transport.link_kind();
            st.last_error = None;
            st.sensitivity = drums.sensitivity;
            st.preset_id = drums.id;
            st.preset_name = drums.name;
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
    let drums0 = engine.cfg.lock().drums.clone();
    let mut seen_gen = engine.config_gen.load(Ordering::SeqCst);
    let mut detector =
        OnsetDetector::new(sample_rate, drums0.sensitivity, drums0.detector.clone());
    let mut mapper = HitMapper::new(MapperConfig {
        settings: drums0.mapper.clone(),
        sensitivity: drums0.sensitivity,
    });
    let mut eng = drums0.engine.clone();

    let mut current_intensity: u8 = eng.base_intensity;
    let mut hits_since_intensity = 0u32;
    let mut reconnect_backoff = Duration::from_millis(250);
    let mut haptic_ready = false;

    while engine.running.load(Ordering::SeqCst) {
        let gen = engine.config_gen.load(Ordering::SeqCst);
    // Apply sample-rate changes by rebuilding detector.
        if gen != seen_gen {
            seen_gen = gen;
            let drums = engine.cfg.lock().drums.clone();
            let sample_rate = engine.cfg.lock().sample_rate;
            detector = OnsetDetector::new(sample_rate, drums.sensitivity, drums.detector.clone());
            mapper.apply_settings(drums.sensitivity, drums.mapper.clone());
            eng = drums.engine.clone();
            current_intensity = eng.base_intensity;
            hits_since_intensity = 0;
            // Keep haptics enabled; only refresh intensity.
            let _ = transport.set_haptic(true, current_intensity);
            haptic_ready = true;
        }

        if !haptic_ready {
            if let Err(e) = transport.set_haptic(true, current_intensity) {
                engine.status.lock().last_error = Some(e.to_string());
            } else {
                haptic_ready = true;
            }
        }

        let sens = engine.cfg.lock().drums.sensitivity;
        detector.set_sensitivity(sens);
        mapper.set_sensitivity(sens);

        let drain = eng.drain_frames.max(128);
        let samples = engine.ring.drain(drain);
        if samples.is_empty() {
            thread::sleep(Duration::from_millis(eng.idle_sleep_ms.max(0)));
            continue;
        }

        let onsets = if samples.len() >= 2 {
            detector.process_interleaved_stereo(&samples)
        } else {
            detector.process_mono(&samples)
        };

        for onset in onsets {
            if let Some(hit) = mapper.map(onset) {
                // Never turn the motor down for weak onsets — that made Hits climb
                // while music felt dead (test pulse still worked at intensity 80).
                let target_intensity = hit
                    .intensity
                    .max(eng.base_intensity)
                    .max(55)
                    .min(100);

                hits_since_intensity += 1;
                let delta =
                    (target_intensity as i16 - current_intensity as i16).unsigned_abs() as u8;
                let should_update = if eng.intensity_update_every_hits <= 1
                    && eng.intensity_delta_threshold == 0
                {
                    target_intensity != current_intensity
                } else {
                    hits_since_intensity >= eng.intensity_update_every_hits
                        || delta > eng.intensity_delta_threshold
                };

                if should_update {
                    current_intensity = target_intensity;
                    hits_since_intensity = 0;
                    if let Err(e) = transport.set_haptic(true, current_intensity) {
                        engine.status.lock().last_error = Some(e.to_string());
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
                    }
                }

                match transport.trigger(hit.pulse) {
                    Ok(()) => {
                        engine.status.lock().hits_fired += 1;
                    }
                    Err(e) => {
                        engine.status.lock().last_error = Some(e.to_string());
                        if let Ok(t) = open_best_transport_with_retry(3, reconnect_backoff) {
                            transport = t;
                            engine.status.lock().link = transport.link_kind();
                            let _ = transport.set_haptic(true, current_intensity);
                            if transport.trigger(hit.pulse).is_ok() {
                                engine.status.lock().hits_fired += 1;
                            }
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
