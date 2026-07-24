//! Configurable onset detection for drum-like haptic triggers.

use crate::config::{DetectorMode, DetectorSettings};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnsetKind {
    Kick,
    Snare,
    Hat,
}

#[derive(Debug, Clone, Copy)]
pub struct Onset {
    pub strength: f32,
    pub kind: OnsetKind,
}

/// Onset detector driven by [`DetectorSettings`] (Classic multiband or Kick gate).
pub struct OnsetDetector {
    sample_rate: f32,
    sensitivity: f32,
    settings: DetectorSettings,
    // Multiband state
    low_env: f32,
    mid_env: f32,
    high_env: f32,
    prev_low: f32,
    prev_mid: f32,
    prev_high: f32,
    // Kick state
    lp: f32,
    lp_alpha: f32,
    fast: f32,
    slow: f32,
    prev_fast: f32,
    broadband: f32,
    // Shared
    cooldown_samples: usize,
    samples_since_hit: usize,
    history: VecDeque<f32>,
}

impl OnsetDetector {
    pub fn new(sample_rate: f32, sensitivity: f32, settings: DetectorSettings) -> Self {
        let sr = sample_rate.max(8_000.0);
        let sensitivity = sensitivity.clamp(0.05, 1.0);
        let lp_alpha = 1.0 - (-2.0 * std::f32::consts::PI * settings.lp_hz.max(1.0) / sr).exp();
        let mut d = Self {
            sample_rate: sr,
            sensitivity,
            settings,
            low_env: 0.0,
            mid_env: 0.0,
            high_env: 0.0,
            prev_low: 0.0,
            prev_mid: 0.0,
            prev_high: 0.0,
            lp: 0.0,
            lp_alpha,
            fast: 0.0,
            slow: 0.0,
            prev_fast: 0.0,
            broadband: 0.0,
            cooldown_samples: 0,
            samples_since_hit: usize::MAX / 4,
            history: VecDeque::with_capacity(128),
        };
        d.recompute_cooldown();
        d
    }

    pub fn apply_settings(&mut self, sensitivity: f32, settings: DetectorSettings) {
        self.sensitivity = sensitivity.clamp(0.05, 1.0);
        self.lp_alpha =
            1.0 - (-2.0 * std::f32::consts::PI * settings.lp_hz.max(1.0) / self.sample_rate).exp();
        self.settings = settings;
        self.recompute_cooldown();
        let cap = self.settings.history_len.max(8);
        while self.history.len() > cap {
            self.history.pop_front();
        }
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.clamp(0.05, 1.0);
        self.recompute_cooldown();
    }

    fn recompute_cooldown(&mut self) {
        let mut ms = self.settings.cooldown_ms;
        if self.settings.cooldown_sens_span_ms > 0.0 {
            ms = (self.settings.cooldown_ms + self.settings.cooldown_sens_span_ms)
                - self.settings.cooldown_sens_span_ms * self.sensitivity;
        }
        self.cooldown_samples = ((self.sample_rate * ms) / 1000.0) as usize;
    }

    pub fn process_interleaved_stereo(&mut self, frames: &[f32]) -> Vec<Onset> {
        let mut out = Vec::new();
        for pair in frames.chunks_exact(2) {
            let mono = 0.5 * (pair[0] + pair[1]);
            if let Some(onset) = self.process_sample(mono) {
                out.push(onset);
            }
        }
        if frames.len() % 2 == 1 {
            if let Some(last) = frames.last() {
                if let Some(onset) = self.process_sample(*last) {
                    out.push(onset);
                }
            }
        }
        out
    }

    pub fn process_mono(&mut self, samples: &[f32]) -> Vec<Onset> {
        samples.iter().filter_map(|s| self.process_sample(*s)).collect()
    }

    fn process_sample(&mut self, sample: f32) -> Option<Onset> {
        match self.settings.mode {
            DetectorMode::Multiband => self.process_multiband(sample),
            DetectorMode::Kick => self.process_kick(sample),
        }
    }

    fn process_multiband(&mut self, sample: f32) -> Option<Onset> {
        self.samples_since_hit = self.samples_since_hit.saturating_add(1);

        let abs = sample.abs();
        self.low_env = self.low_env * 0.995 + abs * 0.005;
        self.mid_env = self.mid_env * 0.98 + abs * 0.02;
        self.high_env = self.high_env * 0.90 + abs * 0.10;

        let low_flux = (self.low_env - self.prev_low).max(0.0);
        let mid_flux = (self.mid_env - self.prev_mid).max(0.0);
        let high_flux = (self.high_env - self.prev_high).max(0.0);
        self.prev_low = self.low_env;
        self.prev_mid = self.mid_env;
        self.prev_high = self.high_env;

        let s = &self.settings;
        let flux = low_flux * s.low_weight + mid_flux * s.mid_weight + high_flux * s.high_weight;
        self.history.push_back(flux);
        while self.history.len() > s.history_len.max(8) {
            self.history.pop_front();
        }
        let mean = if self.history.is_empty() {
            0.0
        } else {
            self.history.iter().sum::<f32>() / self.history.len() as f32
        };
        let threshold = (s.flux_base + mean * s.flux_mean_mul) / self.sensitivity;

        if self.samples_since_hit < self.cooldown_samples {
            return None;
        }
        if flux < threshold {
            return None;
        }

        self.samples_since_hit = 0;
        let strength = ((flux / (threshold + 1e-6)) - 1.0).clamp(0.0, 1.0);
        let kind = if low_flux >= mid_flux && low_flux >= high_flux {
            OnsetKind::Kick
        } else if mid_flux >= high_flux {
            OnsetKind::Snare
        } else {
            OnsetKind::Hat
        };
        Some(Onset { strength, kind })
    }

    fn process_kick(&mut self, sample: f32) -> Option<Onset> {
        self.samples_since_hit = self.samples_since_hit.saturating_add(1);

        let abs = sample.abs();
        self.broadband = self.broadband * 0.995 + abs * 0.005;

        self.lp += self.lp_alpha * (sample - self.lp);
        let bass = self.lp.abs();

        self.fast = self.fast * 0.86 + bass * 0.14;
        self.slow = self.slow * 0.9985 + bass * 0.0015;

        let flux = (self.fast - self.prev_fast).max(0.0);
        self.prev_fast = self.fast;

        let s = &self.settings;
        self.history.push_back(flux);
        while self.history.len() > s.history_len.max(8) {
            self.history.pop_front();
        }
        let mean = if self.history.is_empty() {
            0.0
        } else {
            self.history.iter().sum::<f32>() / self.history.len() as f32
        };

        let sens = self.sensitivity;
        let floor_mul = s.floor_mul_base + (1.0 - sens) * s.floor_mul_sens_span;
        let threshold = (s.flux_base + mean * floor_mul).max(0.0012);

        if self.samples_since_hit < self.cooldown_samples {
            return None;
        }
        if flux < threshold {
            return None;
        }

        let bass_ratio = bass / (self.broadband + 1e-6);
        if bass_ratio < s.bass_ratio_min {
            return None;
        }
        if self.fast < self.slow * (1.15 + (1.0 - sens) * 0.5) && flux < threshold * 1.8 {
            return None;
        }

        self.samples_since_hit = 0;
        let strength = ((flux / (threshold + 1e-6)) - 1.0).clamp(0.0, 1.0);
        let kind = if bass_ratio > 0.85 || strength > 0.25 {
            OnsetKind::Kick
        } else if bass_ratio > 0.65 {
            OnsetKind::Snare
        } else {
            OnsetKind::Hat
        };
        Some(Onset { strength, kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DrumsConfig;

    #[test]
    fn classic_detects_impulse() {
        let cfg = DrumsConfig::classic();
        let mut d = OnsetDetector::new(48_000.0, 0.8, cfg.detector);
        let mut buf = vec![0.0_f32; 4000];
        for s in &mut buf[2000..2050] {
            *s = 0.9;
        }
        let hits = d.process_mono(&buf);
        assert!(!hits.is_empty());
    }

    #[test]
    fn house_detects_bass_impulse() {
        let cfg = DrumsConfig::house();
        let mut d = OnsetDetector::new(48_000.0, 0.7, cfg.detector);
        let mut buf = vec![0.0_f32; 8000];
        for (i, s) in buf.iter_mut().enumerate().take(3000) {
            *s = ((i as f32) * 0.01).sin() * 0.02;
        }
        for i in 4000..4200 {
            let t = (i - 4000) as f32 / 200.0;
            buf[i] = (1.0 - t) * (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.9;
        }
        let hits = d.process_mono(&buf);
        assert!(!hits.is_empty());
    }
}
