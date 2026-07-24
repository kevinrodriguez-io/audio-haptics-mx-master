//! Map musical onsets onto MX Master 4 pulse vocabulary + intensity.

use crate::config::MapperSettings;
use crate::dsp::{Onset, OnsetKind};
use crate::hidpp::{HapticPulse, PulseType};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct MapperConfig {
    pub settings: MapperSettings,
    pub sensitivity: f32,
}

impl Default for MapperConfig {
    fn default() -> Self {
        Self {
            settings: MapperSettings::default(),
            sensitivity: 0.65,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MappedHit {
    pub intensity: u8,
    pub pulse: HapticPulse,
}

pub struct HitMapper {
    cfg: MapperConfig,
    last_fire: Option<Instant>,
}

impl HitMapper {
    pub fn new(cfg: MapperConfig) -> Self {
        let mut m = Self {
            cfg,
            last_fire: None,
        };
        m.recompute_interval();
        m
    }

    pub fn apply_settings(&mut self, sensitivity: f32, settings: MapperSettings) {
        self.cfg.sensitivity = sensitivity.clamp(0.05, 1.0);
        self.cfg.settings = settings;
        self.recompute_interval();
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.cfg.sensitivity = sensitivity.clamp(0.05, 1.0);
        self.recompute_interval();
    }

    fn recompute_interval(&mut self) {
        let s = &self.cfg.settings;
        let mut ms = s.min_interval_ms;
        if s.scale_interval_with_sensitivity {
            ms = (140.0 - 50.0 * self.cfg.sensitivity).clamp(90.0, 160.0);
        }
        // Stored via Duration at map time from settings; keep min_interval_ms as source.
        let _ = ms;
    }

    fn min_interval(&self) -> Duration {
        let s = &self.cfg.settings;
        let ms = if s.scale_interval_with_sensitivity {
            (140.0 - 50.0 * self.cfg.sensitivity).clamp(90.0, 160.0)
        } else {
            s.min_interval_ms
        };
        Duration::from_millis(ms.clamp(20.0, 500.0) as u64)
    }

    pub fn map(&mut self, onset: Onset) -> Option<MappedHit> {
        let now = Instant::now();
        if let Some(last) = self.last_fire {
            if now.duration_since(last) < self.min_interval() {
                return None;
            }
        }

        let s = &self.cfg.settings;
        if s.drop_hats && matches!(onset.kind, OnsetKind::Hat) {
            return None;
        }

        let boosted = (onset.strength
            * (s.strength_scale_base + self.cfg.sensitivity * s.strength_scale_sens))
            .clamp(0.0, 1.0);
        let min_strength =
            s.min_strength_base + (1.0 - self.cfg.sensitivity) * s.min_strength_sens_span;
        if boosted < min_strength {
            return None;
        }

        let span = s.intensity_ceil.saturating_sub(s.intensity_floor) as f32;
        let intensity = if s.pulse_dynamics {
            (s.intensity_floor as f32 + boosted * span)
                .round()
                .clamp(s.intensity_floor as f32, s.intensity_ceil as f32) as u8
        } else {
            s.intensity_floor + (boosted * span).round().clamp(0.0, span) as u8
        };

        let pulse = match onset.kind {
            OnsetKind::Kick => {
                if s.use_compound_kick && boosted > s.kick_strong_at {
                    HapticPulse::compound(PulseType::Strong, PulseType::Tick)
                } else if boosted > s.kick_strong_at {
                    HapticPulse::single(PulseType::Strong)
                } else if boosted > s.kick_tick_at {
                    if s.use_compound_kick {
                        HapticPulse::single(PulseType::Strong)
                    } else {
                        HapticPulse::single(PulseType::Tick)
                    }
                } else if s.pulse_dynamics {
                    // House path: light only when explicitly pulse-dynamic.
                    HapticPulse::single(PulseType::Tick)
                } else {
                    HapticPulse::single(PulseType::Tick)
                }
            }
            OnsetKind::Snare => {
                if boosted > 0.45 {
                    HapticPulse::single(PulseType::Strong)
                } else {
                    HapticPulse::single(PulseType::Tick)
                }
            }
            OnsetKind::Hat => {
                if s.drop_hats {
                    return None;
                }
                // Light is effectively silent on MX4 at normal intensities.
                HapticPulse::single(PulseType::Tick)
            }
        };

        self.last_fire = Some(now);
        Some(MappedHit { intensity, pulse })
    }
}
