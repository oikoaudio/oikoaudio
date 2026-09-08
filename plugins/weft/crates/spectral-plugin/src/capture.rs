//! Audio-owned accumulation with a bounded, lock-free editor snapshot.
use crate::display_data::ANALYZER_POINTS;
use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) struct CaptureDisplay {
    request: AtomicU32,
    epoch: AtomicU32,
    sequence: AtomicU32,
    published_epoch: AtomicU32,
    power: [AtomicU32; ANALYZER_POINTS],
    seconds: AtomicU32,
}

impl Default for CaptureDisplay {
    fn default() -> Self {
        Self {
            request: AtomicU32::new(0),
            epoch: AtomicU32::new(0),
            sequence: AtomicU32::new(0),
            published_epoch: AtomicU32::new(0),
            power: [const { AtomicU32::new(0) }; ANALYZER_POINTS],
            seconds: AtomicU32::new(0),
        }
    }
}

impl CaptureDisplay {
    pub(crate) fn begin(&self) -> u32 {
        let epoch = self
            .epoch
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
            .max(1);
        self.request.store(epoch, Ordering::SeqCst);
        epoch
    }

    pub(crate) fn stop(&self) {
        self.request.store(0, Ordering::SeqCst);
    }

    pub(crate) fn read(&self, epoch: u32) -> Option<([f64; ANALYZER_POINTS], f32)> {
        // Never spin waiting for the audio thread. Keep the previous preview
        // if publication overlaps this read.
        for _ in 0..2 {
            let before = self.sequence.load(Ordering::SeqCst);
            if before & 1 != 0 {
                continue;
            }
            let published_epoch = self.published_epoch.load(Ordering::SeqCst);
            let power = std::array::from_fn(|i| {
                f32::from_bits(self.power[i].load(Ordering::SeqCst)) as f64
            });
            let seconds = f32::from_bits(self.seconds.load(Ordering::SeqCst));
            if before == self.sequence.load(Ordering::SeqCst)
                && published_epoch == epoch
                && seconds > 0.0
            {
                return Some((power, seconds));
            }
        }
        None
    }
}

pub(crate) struct CaptureAccumulator {
    epoch: u32,
    power_sum: [f64; ANALYZER_POINTS],
    seconds: f64,
}

impl Default for CaptureAccumulator {
    fn default() -> Self {
        Self {
            epoch: 0,
            power_sum: [0.0; ANALYZER_POINTS],
            seconds: 0.0,
        }
    }
}

impl CaptureAccumulator {
    pub(crate) fn push(
        &mut self,
        levels: &[f32; ANALYZER_POINTS],
        seconds: f32,
        display: &CaptureDisplay,
    ) {
        let epoch = display.request.load(Ordering::SeqCst);
        if epoch == 0 {
            return;
        }
        if epoch != self.epoch {
            self.epoch = epoch;
            self.power_sum.fill(0.0);
            self.seconds = 0.0;
        }
        let duration = seconds as f64;
        self.seconds += duration;
        for (sum, db) in self.power_sum.iter_mut().zip(levels) {
            *sum += 10.0_f64.powf(*db as f64 / 10.0) * duration;
        }
        display.sequence.fetch_add(1, Ordering::SeqCst);
        for (sum, target) in self.power_sum.iter().zip(&display.power) {
            target.store(((*sum / self.seconds) as f32).to_bits(), Ordering::SeqCst);
        }
        display.published_epoch.store(epoch, Ordering::SeqCst);
        display
            .seconds
            .store((self.seconds as f32).to_bits(), Ordering::SeqCst);
        display.sequence.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests;
