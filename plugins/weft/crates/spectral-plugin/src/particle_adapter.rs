//! Host-to-core particle adapter. Voice slots are retired before reuse; pinned
//! slots are disjoint. No MTS calls or GUI time enter the engine.
use super::*;
use crate::parameters::SpectralMotionShape;
use spectral_dsp::MotionShape;
use spectral_dsp::particles::{Config, Direction, Engine, Shape, Source};

pub(super) struct Adapter {
    pub engine: Engine,
    configured_rate: f32,
    pub pending: [Option<u64>; MAX_VOICES],
    mapped: [bool; MAX_MASK_VOICES],
    pinned: [bool; MIDI_NOTES],
    pub gains: Vec<f32>,
    transition_from: Vec<f32>,
    last_shape: MotionShape,
    previous_motion: MotionConfig,
    transition_remaining: f32,
    pub last_tempo: f32,
    expected_position: Option<f64>,
    published_samples: usize,
}
impl Adapter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            configured_rate: 1.0,
            pending: [None; MAX_VOICES],
            mapped: [false; MAX_MASK_VOICES],
            pinned: [false; MIDI_NOTES],
            gains: vec![1.0; MAX_FFT_SIZE / 2 + 1],
            transition_from: vec![1.0; MAX_FFT_SIZE / 2 + 1],
            last_shape: MotionShape::Drift,
            previous_motion: MotionConfig {
                shape: MotionShape::Drift,
                ..MotionConfig::default()
            },
            transition_remaining: 0.0,
            last_tempo: 120.0,
            expected_position: None,
            published_samples: 0,
        }
    }
    pub fn reset(&mut self) {
        self.engine.reset();
        self.pending.fill(None);
        self.mapped.fill(false);
        self.pinned.fill(false);
        self.gains.fill(1.0);
        self.transition_from.fill(1.0);
        self.last_shape = MotionShape::Drift;
        self.previous_motion = MotionConfig {
            shape: MotionShape::Drift,
            ..MotionConfig::default()
        };
        self.transition_remaining = 0.0;
        self.expected_position = None;
        self.published_samples = 0;
        self.last_tempo = 120.0;
    }
    pub fn transport(
        &mut self,
        position: Option<f64>,
        playing: bool,
        samples: usize,
        sample_rate: f32,
        sync: bool,
    ) {
        let position = position.filter(|p| p.is_finite());
        if let Some(position) = position {
            let tolerance = 2.0 * self.last_tempo as f64 / (60.0 * sample_rate as f64);
            let discontinuity = self
                .expected_position
                .is_some_and(|expected| (position - expected).abs() > tolerance);
            if discontinuity || (sync && self.expected_position.is_none()) {
                self.engine
                    .seek(position * 60.0 / self.last_tempo as f64 * self.configured_rate as f64);
            }
            self.expected_position = Some(
                position
                    + if playing {
                        samples as f64 / sample_rate as f64 * self.last_tempo as f64 / 60.0
                    } else {
                        0.0
                    },
            );
        } else {
            self.expected_position = None;
        }
    }
    pub fn publish(&mut self, display: &AnalysisDisplay, samples: usize, bins: usize, bin_hz: f32) {
        self.published_samples += samples;
        if self.published_samples >= (display.sample_rate() / 30.0) as usize {
            display.store_particles(
                &self.gains[..bins],
                bin_hz,
                self.last_shape.is_particle() || self.transition_remaining > 0.0,
            );
            self.published_samples = 0;
        }
    }
    /// Apply one common motion field after ordinary note/manual masks. Freeze
    /// the previous field at mode changes, then crossfade for 50 ms even with
    /// Smooth disabled. Returns whether `apply` must run this frame; outside a
    /// transition, looping shapes are applied by the ordinary mask instead.
    pub fn begin_frame(&mut self, config: MotionConfig, bin_hz: f32, bins: usize) -> bool {
        if config.shape != self.last_shape {
            if config.shape.is_particle()
                || self.last_shape.is_particle()
                || self.transition_remaining > 0.0
            {
                if !self.last_shape.is_particle() && self.transition_remaining <= 0.0 {
                    let compensation = spectral_dsp::motion_compensation_gain(self.previous_motion);
                    for (bin, gain) in self.gains[..bins].iter_mut().enumerate() {
                        *gain = if self.previous_motion.depth_db > 0.001 {
                            db_to_gain(-spectral_dsp::motion_attenuation_db(
                                bin as f32 * bin_hz,
                                self.previous_motion,
                            )) * compensation
                        } else {
                            1.0
                        };
                    }
                }
                self.transition_from.copy_from_slice(&self.gains);
                self.transition_remaining = spectral_dsp::particles::TRANSITION_SECONDS;
            }
            self.last_shape = config.shape;
        }
        config.shape.is_particle() || self.transition_remaining > 0.0
    }
    pub fn apply(
        &mut self,
        config: MotionConfig,
        seconds: f32,
        bin_hz: f32,
        particle_gains: &[f32],
        left: &mut [f32],
        right: Option<&mut [f32]>,
    ) {
        self.previous_motion = config;
        if !config.shape.is_particle() && self.transition_remaining <= 0.0 {
            return;
        }
        let mix = if self.transition_remaining > 0.0 {
            self.transition_remaining = (self.transition_remaining - seconds).max(0.0);
            1.0 - self.transition_remaining / spectral_dsp::particles::TRANSITION_SECONDS
        } else {
            1.0
        };
        let compensation = spectral_dsp::motion_compensation_gain(config);
        for (bin, left) in left.iter_mut().enumerate() {
            let desired = if config.shape.is_particle() {
                particle_gains[bin]
            } else if config.depth_db > 0.001 {
                db_to_gain(-spectral_dsp::motion_attenuation_db(
                    bin as f32 * bin_hz,
                    config,
                )) * compensation
            } else {
                1.0
            };
            let gain = if config.depth_db <= 0.0 {
                1.0
            } else if mix >= 1.0 {
                desired
            } else {
                self.transition_from[bin] + (desired - self.transition_from[bin]) * mix
            };
            self.gains[bin] = gain;
            *left *= gain;
        }
        if let Some(right) = right {
            for (bin, gain) in right.iter_mut().enumerate() {
                *gain *= self.gains[bin];
            }
        }
    }
}

impl SpectralPlugin {
    pub(super) fn configure_particles(&mut self, tempo: f32, scheduled: bool) {
        let rate = if self.params.motion_sync.value() {
            self.params.motion_rate_division.value().rate_hz(tempo)
        } else {
            self.params.motion_rate_hz.value()
        };
        self.particles.configured_rate =
            effective_motion_rate_hz(rate, self.sample_rate, self.quality.size());
        self.particles.engine.configure(Config {
            enabled: MotionShape::from(self.params.motion_shape.value()).is_particle(),
            shape: match self.params.motion_shape.value() {
                SpectralMotionShape::Cloud => Shape::Cloud,
                _ => Shape::Sprinkle,
            },
            direction: match self.params.motion_direction.value() {
                MotionDirection::Forward => Direction::Forward,
                MotionDirection::Reverse => Direction::Reverse,
                MotionDirection::Alternate => Direction::Alternate,
            },
            rate_hz: effective_motion_rate_hz(rate, self.sample_rate, self.quality.size()),
            size_octaves: self.params.motion_size_octaves.value(),
            phase: self.params.motion_phase_percent.value() * 0.01,
            scheduled: !self.params.motion_sync.value() || scheduled,
            sync: self.params.motion_sync.value(),
            hop_samples: self.quality.size() / OVERLAP_TIMES,
            partials: self.params.partials.value() as usize,
            partial_rolloff_db: self.params.harmonic_rolloff_db.value(),
            attack_ms: self.params.note_attack_ms.value(),
            release_ms: self.params.note_release_ms.value(),
        });
    }
    pub(super) fn update_particle_sources(&mut self) {
        self.particles.engine.set_note_input_present(
            self.voices
                .iter()
                .any(|v| v.occupied && (v.held || v.sustained))
                || (0..MIDI_NOTES).any(|note| self.params.pinned_notes.get(note)),
        );
        self.midi_expression.resolve(
            &mut self.voices,
            self.params.pitch_bend_range.value() as f32,
        );
        let sensitivity = self
            .params
            .velocity_sensitivity_percent
            .smoothed
            .previous_value()
            * 0.01;
        let vibrato =
            (std::f32::consts::TAU * self.expression_phase).sin() * VIBRATO_RANGE_SEMITONES;
        for (slot, voice) in self.voices.iter().enumerate() {
            let mapped = voice.occupied && self.tuning.mapped[voice.note as usize];
            if self.particles.mapped[slot] && !mapped && voice.occupied {
                self.particles.engine.retire_source(slot);
            }
            self.particles.mapped[slot] = mapped;
            if voice.occupied && mapped {
                self.particles.engine.set_source(
                    slot,
                    Source {
                        frequency_hz: self.tuning.frequencies[voice.note as usize]
                            * 2.0_f32.powf(
                                (voice.expression.tuning_semitones + vibrato * voice.vibrato)
                                    / 12.0,
                            ),
                        strength: effective_velocity(voice.velocity, sensitivity),
                        eligible: (voice.held || voice.sustained) && voice.expression.gain > 0.0,
                    },
                );
            } else {
                self.particles.engine.release_source(slot);
            }
            if let Some(index) = self.particles.pending[slot].take() {
                self.particles.engine.trigger_reserved(slot, index);
            }
        }
        for note in 0..MIDI_NOTES {
            let slot = MAX_VOICES + note;
            let pinned = self.params.pinned_notes.get(note) && self.tuning.mapped[note];
            if self.particles.pinned[note] && !pinned {
                self.particles.engine.retire_source(slot);
            }
            self.particles.pinned[note] = pinned;
            self.particles.engine.set_source(
                slot,
                Source {
                    frequency_hz: self.tuning.frequencies[note],
                    strength: effective_velocity(PINNED_NOTE_VELOCITY, sensitivity),
                    eligible: pinned,
                },
            );
        }
    }
}
