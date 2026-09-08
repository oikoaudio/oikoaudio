//! Bounded atomic audio-to-UI display data. No graphics or host dependencies.
//! Scalar metering is approximate; it must never drive sound processing.
use crate::MAX_SPLASH_EVENTS;
use spectral_dsp::{MANUAL_CURVE_MUTE_DB, MIDI_NOTES, MIN_DISPLAY_FREQUENCY_HZ, SplashEvent};
use std::sync::atomic::{AtomicU32, Ordering};
pub(crate) const ANALYZER_POINTS: usize = 256;

const MAX_DISPLAY_FREQUENCY_HZ: f32 = 22_000.0;
pub(crate) fn display_max_frequency(sample_rate: f32) -> f32 {
    (sample_rate * 0.5).clamp(MIN_DISPLAY_FREQUENCY_HZ, MAX_DISPLAY_FREQUENCY_HZ)
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
    pub(crate) splash_centers: [AtomicU32; MAX_SPLASH_EVENTS],
    pub(crate) splash_radii: [AtomicU32; MAX_SPLASH_EVENTS],
    pub(crate) splash_strengths: [AtomicU32; MAX_SPLASH_EVENTS],
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
            splash_centers: [const { AtomicU32::new(0.0_f32.to_bits()) }; MAX_SPLASH_EVENTS],
            splash_radii: [const { AtomicU32::new(0.0_f32.to_bits()) }; MAX_SPLASH_EVENTS],
            splash_strengths: [const { AtomicU32::new(0.0_f32.to_bits()) }; MAX_SPLASH_EVENTS],
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

    pub(crate) fn store_splashes(&self, splashes: &[SplashEvent; MAX_SPLASH_EVENTS]) {
        for (index, splash) in splashes.iter().enumerate() {
            self.splash_centers[index].store(splash.center_hz.to_bits(), Ordering::Release);
            self.splash_radii[index].store(splash.radius_octaves.to_bits(), Ordering::Release);
            self.splash_strengths[index].store(splash.strength.to_bits(), Ordering::Release);
        }
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
        for index in 0..MAX_SPLASH_EVENTS {
            self.splash_centers[index].store(0.0_f32.to_bits(), Ordering::Release);
            self.splash_radii[index].store(0.0_f32.to_bits(), Ordering::Release);
            self.splash_strengths[index].store(0.0_f32.to_bits(), Ordering::Release);
        }
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

    pub(crate) fn splashes(&self) -> [SplashEvent; MAX_SPLASH_EVENTS] {
        std::array::from_fn(|index| SplashEvent {
            center_hz: f32::from_bits(self.splash_centers[index].load(Ordering::Acquire)),
            radius_octaves: f32::from_bits(self.splash_radii[index].load(Ordering::Acquire)),
            strength: f32::from_bits(self.splash_strengths[index].load(Ordering::Acquire)),
        })
    }
}
