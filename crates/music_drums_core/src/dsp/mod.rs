//! Onset / low-band energy detection for drum-like haptic triggers.

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

/// Simple spectral-flux-ish onset detector tuned for music drums MVP.
pub struct OnsetDetector {
    #[allow(dead_code)]
    sample_rate: f32,
    sensitivity: f32,
    low_env: f32,
    mid_env: f32,
    high_env: f32,
    prev_low: f32,
    prev_mid: f32,
    prev_high: f32,
    cooldown_samples: usize,
    samples_since_hit: usize,
    history: VecDeque<f32>,
}

impl OnsetDetector {
    pub fn new(sample_rate: f32, sensitivity: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(8_000.0),
            sensitivity: sensitivity.clamp(0.05, 1.0),
            low_env: 0.0,
            mid_env: 0.0,
            high_env: 0.0,
            prev_low: 0.0,
            prev_mid: 0.0,
            prev_high: 0.0,
            cooldown_samples: (sample_rate * 0.08) as usize,
            samples_since_hit: usize::MAX / 4,
            history: VecDeque::with_capacity(64),
        }
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.clamp(0.05, 1.0);
    }

    /// Process interleaved stereo f32 samples; returns onsets detected in this block.
    pub fn process_interleaved_stereo(&mut self, frames: &[f32]) -> Vec<Onset> {
        let mut out = Vec::new();
        for pair in frames.chunks_exact(2) {
            let mono = 0.5 * (pair[0] + pair[1]);
            if let Some(onset) = self.process_sample(mono) {
                out.push(onset);
            }
        }
        // Also accept mono buffers.
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
        self.samples_since_hit = self.samples_since_hit.saturating_add(1);

        let abs = sample.abs();
        // Crude band proxies via envelope speeds (not true filters, good enough for MVP).
        self.low_env = self.low_env * 0.995 + abs * 0.005;
        self.mid_env = self.mid_env * 0.98 + abs * 0.02;
        self.high_env = self.high_env * 0.90 + abs * 0.10;

        let low_flux = (self.low_env - self.prev_low).max(0.0);
        let mid_flux = (self.mid_env - self.prev_mid).max(0.0);
        let high_flux = (self.high_env - self.prev_high).max(0.0);
        self.prev_low = self.low_env;
        self.prev_mid = self.mid_env;
        self.prev_high = self.high_env;

        let flux = low_flux * 1.6 + mid_flux * 1.1 + high_flux * 0.7;
        self.history.push_back(flux);
        if self.history.len() > 48 {
            self.history.pop_front();
        }
        let mean = if self.history.is_empty() {
            0.0
        } else {
            self.history.iter().sum::<f32>() / self.history.len() as f32
        };
        let threshold = (0.002 + mean * 2.5) / self.sensitivity;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_impulse() {
        let mut d = OnsetDetector::new(48_000.0, 0.8);
        let mut buf = vec![0.0_f32; 4000];
        for s in &mut buf[2000..2050] {
            *s = 0.9;
        }
        let hits = d.process_mono(&buf);
        assert!(!hits.is_empty());
    }
}
