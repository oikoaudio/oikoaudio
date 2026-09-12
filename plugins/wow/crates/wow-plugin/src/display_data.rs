//! Bounded atomic audio-to-UI display data. No graphics or host dependencies.
//! Scalar metering is approximate; it must never drive sound processing.
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
pub(crate) const DISPLAY_REFRESH_HZ: f64 = 480.0;
const DISPLAY_POINTS: usize = 2048;

pub(crate) struct ModulationDisplay {
    pub(crate) left: [AtomicU32; DISPLAY_POINTS],
    pub(crate) right: [AtomicU32; DISPLAY_POINTS],
    tempo_bpm: AtomicU32,
    pub(crate) cursor: AtomicUsize,
}

impl Default for ModulationDisplay {
    fn default() -> Self {
        Self {
            left: [const { AtomicU32::new(0) }; DISPLAY_POINTS],
            right: [const { AtomicU32::new(0) }; DISPLAY_POINTS],
            tempo_bpm: AtomicU32::new(120.0_f32.to_bits()),
            cursor: AtomicUsize::new(0),
        }
    }
}

impl ModulationDisplay {
    // Independent approximate scalar: the audio thread writes and the editor reads.
    pub(crate) fn store_tempo(&self, tempo: f32) {
        self.tempo_bpm.store(tempo.to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn tempo(&self) -> f32 {
        f32::from_bits(self.tempo_bpm.load(Ordering::Relaxed))
    }

    #[inline]
    pub(crate) fn push(&self, left: f32, right: f32) {
        let sequence = self.cursor.load(Ordering::Relaxed);
        let slot = sequence % DISPLAY_POINTS;
        self.left[slot].store(left.to_bits(), Ordering::Relaxed);
        self.right[slot].store(right.to_bits(), Ordering::Relaxed);
        self.cursor
            .store(sequence.wrapping_add(1), Ordering::Release);
    }

    pub(crate) fn clear(&self) {
        self.cursor.store(0, Ordering::Release);
    }

    pub(crate) fn snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        let cursor = self.cursor.load(Ordering::Acquire);
        let count = cursor.min(DISPLAY_POINTS);
        let start = cursor.saturating_sub(count);
        let mut left = Vec::with_capacity(count);
        let mut right = Vec::with_capacity(count);
        for sequence in start..cursor {
            let slot = sequence % DISPLAY_POINTS;
            left.push(f32::from_bits(self.left[slot].load(Ordering::Relaxed)));
            right.push(f32::from_bits(self.right[slot].load(Ordering::Relaxed)));
        }
        (left, right)
    }
}

#[cfg(test)]
mod tests;
