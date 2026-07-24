//! Map musical onsets onto MX Master 4 pulse vocabulary + intensity.

use crate::dsp::{Onset, OnsetKind};
use crate::hidpp::{HapticPulse, PulseType};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct MapperConfig {
    pub min_interval: Duration,
    pub intensity_floor: u8,
    pub intensity_ceil: u8,
    pub sensitivity: f32,
}

impl Default for MapperConfig {
    fn default() -> Self {
        Self {
            min_interval: Duration::from_millis(70),
            intensity_floor: 20,
            intensity_ceil: 100,
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
        Self {
            cfg,
            last_fire: None,
        }
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.cfg.sensitivity = sensitivity.clamp(0.05, 1.0);
    }

    pub fn map(&mut self, onset: Onset) -> Option<MappedHit> {
        let now = Instant::now();
        if let Some(last) = self.last_fire {
            if now.duration_since(last) < self.cfg.min_interval {
                return None;
            }
        }

        let boosted = (onset.strength * (0.5 + self.cfg.sensitivity)).clamp(0.0, 1.0);
        if boosted < 0.08 {
            return None;
        }

        let span = self.cfg.intensity_ceil.saturating_sub(self.cfg.intensity_floor) as f32;
        let intensity =
            self.cfg.intensity_floor + (boosted * span).round().clamp(0.0, span) as u8;

        let pulse = match onset.kind {
            OnsetKind::Kick => {
                if boosted > 0.75 {
                    HapticPulse::compound(PulseType::Strong, PulseType::Tick)
                } else if boosted > 0.4 {
                    HapticPulse::single(PulseType::Strong)
                } else {
                    HapticPulse::single(PulseType::Tick)
                }
            }
            OnsetKind::Snare => {
                if boosted > 0.55 {
                    HapticPulse::single(PulseType::Tick)
                } else {
                    HapticPulse::single(PulseType::Light)
                }
            }
            OnsetKind::Hat => HapticPulse::single(PulseType::Light),
        };

        self.last_fire = Some(now);
        Some(MappedHit { intensity, pulse })
    }
}
