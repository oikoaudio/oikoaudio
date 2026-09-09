//! Bounded atomic audio-to-UI display data. No graphics or host dependencies.
//! Scalar metering is approximate; it must never drive sound processing.

const MAX_DISPLAY_FREQUENCY_HZ: f32 = 22_000.0;
pub(crate) fn display_max_frequency(sample_rate: f32) -> f32 {
    (sample_rate * 0.5).clamp(MIN_DISPLAY_FREQUENCY_HZ, MAX_DISPLAY_FREQUENCY_HZ)
}

use spectral_dsp::{MANUAL_CURVE_MUTE_DB, MIDI_NOTES, MIN_DISPLAY_FREQUENCY_HZ};
use std::sync::atomic::{AtomicU32, Ordering};
pub(crate) const ANALYZER_POINTS: usize = 256;
const PARTICLE_BINS: usize = crate::parameters::MAX_FFT_SIZE / 2 + 1;
/// Coherent decimated copy of the actual motion multiplier, including mode
/// crossfades and smoothed Depth. UI interpolation does not add FFT resolution.
pub(crate) struct ParticleMask {
    gains: [f32; PARTICLE_BINS],
    bins: usize,
    bin_hz: f32,
    pub active: bool,
}
impl Default for ParticleMask {
    fn default() -> Self {
        Self {
            gains: [1.0; PARTICLE_BINS],
            bins: 0,
            bin_hz: 1.0,
            active: false,
        }
    }
}
impl ParticleMask {
    pub fn attenuation_db(&self, frequency: f32) -> f32 {
        if self.bins == 0 {
            return 0.0;
        }
        let position = (frequency / self.bin_hz).clamp(0.0, (self.bins - 1) as f32);
        let left = position as usize;
        let right = (left + 1).min(self.bins - 1);
        let gain =
            self.gains[left] + (self.gains[right] - self.gains[left]) * (position - left as f32);
        -20.0 * gain.max(1e-9).log10()
    }
}

pub(crate) struct AnalysisDisplay {
    pub(crate) tuning_snapshot: crate::mts_client::Snapshot,
    pub(crate) capture: crate::capture::CaptureDisplay,
    pub(crate) output_peak: AtomicU32,
    pub(crate) note_levels: [AtomicU32; MIDI_NOTES],
    pub(crate) note_tunings: [AtomicU32; MIDI_NOTES],
    pub(crate) note_timbres: [AtomicU32; MIDI_NOTES],
    pub(crate) spectrum_db: [AtomicU32; ANALYZER_POINTS],
    pub(crate) sample_rate: AtomicU32,
    pub(crate) tempo_bpm: AtomicU32,
    pub(crate) motion_phase: AtomicU32,
    particle_generation: AtomicU32,
    particle_words: [AtomicU32; PARTICLE_BINS + 3],
}

impl Default for AnalysisDisplay {
    fn default() -> Self {
        Self {
            capture: crate::capture::CaptureDisplay::default(),
            tuning_snapshot: crate::mts_client::Snapshot::default(),
            output_peak: AtomicU32::new(0),
            note_levels: [const { AtomicU32::new(0.0_f32.to_bits()) }; MIDI_NOTES],
            note_tunings: [const { AtomicU32::new(0.0_f32.to_bits()) }; MIDI_NOTES],
            note_timbres: [const { AtomicU32::new(0.5_f32.to_bits()) }; MIDI_NOTES],
            spectrum_db: [const { AtomicU32::new(MANUAL_CURVE_MUTE_DB.to_bits()) };
                ANALYZER_POINTS],
            sample_rate: AtomicU32::new(48_000.0_f32.to_bits()),
            tempo_bpm: AtomicU32::new(120.0_f32.to_bits()),
            motion_phase: AtomicU32::new(0.0_f32.to_bits()),
            particle_generation: AtomicU32::new(0),
            particle_words: [const { AtomicU32::new(0) }; PARTICLE_BINS + 3],
        }
    }
}

impl AnalysisDisplay {
    pub(crate) fn store_output_peak(&self, peak: f32) {
        self.output_peak
            .fetch_max(peak.to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn take_output_peak(&self) -> f32 {
        f32::from_bits(self.output_peak.swap(0, Ordering::Relaxed))
    }
    pub(crate) fn set_sample_rate(&self, sample_rate: f32) {
        self.sample_rate
            .store(sample_rate.to_bits(), Ordering::Release);
    }

    pub(crate) fn store_tempo(&self, tempo_bpm: f32) {
        self.tempo_bpm.store(tempo_bpm.to_bits(), Ordering::Release);
    }

    pub(crate) fn tempo(&self) -> f32 {
        f32::from_bits(self.tempo_bpm.load(Ordering::Acquire))
    }

    pub(crate) fn store_notes(
        &self,
        levels: &[f32; MIDI_NOTES],
        tunings: &[f32; MIDI_NOTES],
        timbres: &[f32; MIDI_NOTES],
    ) {
        for (target, level) in self.note_levels.iter().zip(levels) {
            target.store(level.to_bits(), Ordering::Release);
        }
        for (target, tuning) in self.note_tunings.iter().zip(tunings) {
            target.store(tuning.to_bits(), Ordering::Release);
        }
        for (target, timbre) in self.note_timbres.iter().zip(timbres) {
            target.store(timbre.to_bits(), Ordering::Release);
        }
    }

    pub(crate) fn store_spectrum(&self, spectrum_db: &[f32; ANALYZER_POINTS]) {
        for (target, level) in self.spectrum_db.iter().zip(spectrum_db.iter()) {
            target.store(level.to_bits(), Ordering::Release);
        }
    }

    pub(crate) fn store_motion_phase(&self, phase: f32) {
        self.motion_phase.store(phase.to_bits(), Ordering::Release);
    }

    /// Single audio writer, coherent seqlock observation. Publication never
    /// waits or retries; UI rejects an overlapping read and retains its frame.
    pub(crate) fn store_particles(&self, gains: &[f32], bin_hz: f32, active: bool) {
        self.particle_generation.fetch_add(1, Ordering::SeqCst);
        for (word, gain) in self.particle_words.iter().zip(gains) {
            word.store(gain.to_bits(), Ordering::SeqCst);
        }
        self.particle_words[PARTICLE_BINS].store(gains.len() as u32, Ordering::SeqCst);
        self.particle_words[PARTICLE_BINS + 1].store(bin_hz.to_bits(), Ordering::SeqCst);
        self.particle_words[PARTICLE_BINS + 2].store(u32::from(active), Ordering::SeqCst);
        self.particle_generation.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn clear(&self) {
        self.output_peak.store(0, Ordering::Relaxed);
        for level in &self.note_levels {
            level.store(0.0_f32.to_bits(), Ordering::Release);
        }
        for tuning in &self.note_tunings {
            tuning.store(0.0_f32.to_bits(), Ordering::Release);
        }
        for timbre in &self.note_timbres {
            timbre.store(0.5_f32.to_bits(), Ordering::Release);
        }
        for level in &self.spectrum_db {
            level.store(MANUAL_CURVE_MUTE_DB.to_bits(), Ordering::Release);
        }
        self.motion_phase
            .store(0.0_f32.to_bits(), Ordering::Release);
        self.store_particles(&[], 1.0, false);
    }

    pub(crate) fn note_level(&self, note: usize) -> f32 {
        f32::from_bits(self.note_levels[note].load(Ordering::Acquire))
    }

    pub(crate) fn note_tuning(&self, note: usize) -> f32 {
        f32::from_bits(self.note_tunings[note].load(Ordering::Acquire))
    }

    pub(crate) fn note_timbre(&self, note: usize) -> f32 {
        f32::from_bits(self.note_timbres[note].load(Ordering::Acquire))
    }

    pub(crate) fn spectrum_level(&self, index: usize) -> f32 {
        f32::from_bits(self.spectrum_db[index].load(Ordering::Acquire))
    }

    pub(crate) fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::Acquire))
    }

    pub(crate) fn motion_phase(&self) -> f32 {
        f32::from_bits(self.motion_phase.load(Ordering::Acquire))
    }

    pub(crate) fn read_particles(&self, output: &mut ParticleMask) {
        let generation = self.particle_generation.load(Ordering::SeqCst);
        if generation & 1 != 0 {
            return;
        }
        let mut candidate = ParticleMask {
            bins: (self.particle_words[PARTICLE_BINS].load(Ordering::SeqCst) as usize)
                .min(PARTICLE_BINS),
            bin_hz: f32::from_bits(self.particle_words[PARTICLE_BINS + 1].load(Ordering::SeqCst)),
            active: self.particle_words[PARTICLE_BINS + 2].load(Ordering::SeqCst) != 0,
            ..ParticleMask::default()
        };
        for (gain, word) in candidate.gains[..candidate.bins]
            .iter_mut()
            .zip(&self.particle_words)
        {
            *gain = f32::from_bits(word.load(Ordering::SeqCst));
        }
        if generation == self.particle_generation.load(Ordering::SeqCst) {
            *output = candidate;
        }
    }
}

#[cfg(test)]
mod tests;
