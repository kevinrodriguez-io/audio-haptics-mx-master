//! Audio input pathway.
//!
//! Process Tap setup uses Apple's `CATapDescription` (Objective-C). The Swift app
//! owns the tap and pushes interleaved stereo f32 PCM into the engine via FFI.
//! This module defines the shared sample format and a ring-buffer helper.

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone)]
pub struct AudioRing {
    inner: Arc<Mutex<VecDeque<f32>>>,
    capacity: usize,
}

impl AudioRing {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(capacity_samples))),
            capacity: capacity_samples,
        }
    }

    pub fn push_interleaved(&self, samples: &[f32]) {
        let mut q = self.inner.lock();
        for &s in samples {
            if q.len() >= self.capacity {
                q.pop_front();
            }
            q.push_back(s);
        }
    }

    pub fn drain(&self, max: usize) -> Vec<f32> {
        let mut q = self.inner.lock();
        let n = max.min(q.len());
        q.drain(..n).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }
}

/// Placeholder documenting that Process Tap lives on the Swift side for TCC.
#[cfg(target_os = "macos")]
pub mod process_tap_notes {
    //! Swift `AudioTap.swift` creates a global process tap and calls
    //! `md_push_audio_frames`. See docs/architecture.md.
}
