use state::PinnedNotesState;
mod curve;
mod parameters;
use curve::transform_curve;
use display_data::{ANALYZER_POINTS, AnalysisDisplay, display_max_frequency};
use parameters::{
    COARSE_FFT_SIZE, DEFAULT_FFT_SIZE, FftQuality, MAX_FFT_SIZE, MAX_FREE_MOTION_RATE_HZ,
    MIN_FFT_SIZE, MotionDirection, ROUGH_FFT_SIZE, SpectralParams,
};
#[cfg(test)]
use spectral_dsp::processing::build_dual_synthesis_window;
use spectral_dsp::processing::{PreparedSpectrum, smooth_mask_in_db, soften_spectral_edges};
mod capture;
mod expression;
#[cfg(test)]
mod expression_tests;
mod particle_adapter;
mod state;
use expression::{MidiExpression, VoiceExpression};
mod curve_transfer;
mod display_data;
mod editor;
mod mts_client;

use editor::{EDITOR_HEIGHT, EDITOR_WIDTH, SpectralEditor, closest_ui_scale};
use nice_plug::midi::{Channel, Key, VoiceID};
use nice_plug::prelude::*;
use nice_plug_egui::{
    EguiEditor, EguiEditorState, EguiNiceSettings, RepaintNotifier, create_egui_editor,
};
use oiko_dsp::db_to_gain;
use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use spectral_dsp::{
    MANUAL_CURVE_MUTE_DB, MANUAL_MASK_POINTS, MIDI_NOTES, MaskConfig, MaskVoice, MotionConfig,
    build_mask_with_voices_precomputed,
};
use std::num::NonZeroU32;
use std::sync::Arc;

const OVERLAP_TIMES: usize = 4;
const MOTION_RATE_FRACTION_OF_FRAME_RATE: f32 = 0.32;
const MAX_VOICES: usize = 128;
const MAX_MASK_VOICES: usize = MAX_VOICES + MIDI_NOTES;
const VIBRATO_RATE_HZ: f32 = 5.5;
const VIBRATO_RANGE_SEMITONES: f32 = 0.5;
const PINNED_NOTE_VELOCITY: f32 = 0.5;

pub(crate) fn maximum_motion_rate_hz(sample_rate: f32, fft_size: usize) -> f32 {
    let frame_rate = sample_rate.max(1.0) * OVERLAP_TIMES as f32 / fft_size.max(1) as f32;
    (frame_rate * MOTION_RATE_FRACTION_OF_FRAME_RATE).clamp(0.01, MAX_FREE_MOTION_RATE_HZ)
}

fn effective_motion_rate_hz(requested_hz: f32, sample_rate: f32, fft_size: usize) -> f32 {
    requested_hz
        .max(0.01)
        .min(maximum_motion_rate_hz(sample_rate, fft_size))
}

struct FftPlan {
    prepared: PreparedSpectrum,
    particle_mask: spectral_dsp::particles::Mask,
    particle_gains: Vec<f32>,
    forward_scratch: Vec<Complex32>,
    inverse_scratch: Vec<Complex32>,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
}

#[derive(Clone, Copy)]
struct VoiceState {
    occupied: bool,
    held: bool,
    sustained: bool,
    voice_id: Option<i32>,
    channel: u8,
    note: u8,
    velocity: f32,
    level: f32,
    tuning_semitones: f32,
    native_pressure: Option<f32>,
    native_timbre: Option<f32>,
    volume_gain: f32,
    pan: f32,
    expression_amount: f32,
    expression: VoiceExpression,
    vibrato: f32,
}

impl VoiceState {
    const EMPTY: Self = Self {
        occupied: false,
        held: false,
        sustained: false,
        voice_id: None,
        channel: 0,
        note: 0,
        velocity: 1.0,
        level: 0.0,
        tuning_semitones: 0.0,
        native_pressure: None,
        native_timbre: None,
        volume_gain: 1.0,
        pan: 0.0,
        expression_amount: 1.0,
        expression: VoiceExpression::DEFAULT,
        vibrato: 0.0,
    };
}

struct AudioSlice<'a, 'b> {
    channels: &'a mut [&'b mut [f32]],
    start: usize,
    len: usize,
}
impl nice_plug::util::StftInput for AudioSlice<'_, '_> {
    fn num_samples(&self) -> usize {
        self.len
    }
    fn num_channels(&self) -> usize {
        self.channels.len()
    }
    unsafe fn get_sample_unchecked(&self, channel: usize, sample: usize) -> f32 {
        self.channels[channel][self.start + sample]
    }
}
impl nice_plug::util::StftInputMut for AudioSlice<'_, '_> {
    unsafe fn get_sample_unchecked_mut(&mut self, channel: usize, sample: usize) -> &mut f32 {
        &mut self.channels[channel][self.start + sample]
    }
}

pub struct SpectralPlugin {
    params: Arc<SpectralParams>,
    editor_state: Arc<EguiEditorState>,
    analysis_display: Arc<AnalysisDisplay>,
    capture_accumulator: capture::CaptureAccumulator,
    sample_rate: f32,
    num_channels: usize,
    quality: FftQuality,
    stft: util::StftHelper,
    plans: Option<[FftPlan; 5]>,
    fft_buffer: Vec<Complex32>,
    mask: Vec<f32>,
    mask_right: Vec<f32>,
    target_mask_right: Vec<f32>,
    softened_mask_right: Vec<f32>,
    hop_position: usize,
    terminated: [VoiceState; MAX_VOICES],
    terminated_count: usize,
    // Expressions may precede note-on within the same sample group. Retain one
    // group in bounded storage; overflow keeps the most recent voice addresses.
    expression_events: [Option<NoteEvent<()>>; MAX_VOICES * 7],
    expression_event_count: usize,
    target_mask: Vec<f32>,
    softened_mask: Vec<f32>,
    analyzer_accumulator: Vec<f32>,
    analyzer_points_db: [f32; ANALYZER_POINTS],
    curve_tilt_octaves: [f32; MANUAL_MASK_POINTS],
    base_curve_cache: [f32; MANUAL_MASK_POINTS],
    manual_curve_cache: [f32; MANUAL_MASK_POINTS],
    voices: [VoiceState; MAX_VOICES],
    particles: particle_adapter::Adapter,
    mask_voices: [MaskVoice; MAX_MASK_VOICES],
    pinned_note_levels: [f32; MIDI_NOTES],
    note_levels: [f32; MIDI_NOTES],
    displayed_note_levels: [f32; MIDI_NOTES],
    note_level_history: [[f32; MIDI_NOTES]; OVERLAP_TIMES],
    note_tunings: [f32; MIDI_NOTES],
    note_timbres: [f32; MIDI_NOTES],
    midi_expression: MidiExpression,
    channel_sustain: [bool; 16],
    motion_phase: f32,
    motion_phase_offset: f32,
    expression_phase: f32,
    hold_was_enabled: bool,
    mts_client: mts_client::Client,
    tuning: mts_client::Tuning,
}

impl Default for SpectralPlugin {
    fn default() -> Self {
        let params = Arc::new(SpectralParams::default());
        Self {
            params,
            editor_state: EguiEditorState::from_size(
                nice_plug::editor::dpi::LogicalSize {
                    width: EDITOR_WIDTH,
                    height: EDITOR_HEIGHT,
                },
                1.0,
            ),
            analysis_display: Arc::new(AnalysisDisplay::default()),
            capture_accumulator: capture::CaptureAccumulator::default(),
            sample_rate: 48_000.0,
            num_channels: 2,
            quality: FftQuality::Balanced,
            stft: util::StftHelper::new(2, MAX_FFT_SIZE, 0),
            plans: None,
            fft_buffer: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            mask: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            mask_right: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            target_mask_right: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            softened_mask_right: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            hop_position: 0,
            terminated: [VoiceState::EMPTY; MAX_VOICES],
            terminated_count: 0,
            expression_events: [None; MAX_VOICES * 7],
            expression_event_count: 0,
            target_mask: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            softened_mask: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            analyzer_accumulator: Vec::with_capacity(MAX_FFT_SIZE / 2 + 1),
            analyzer_points_db: [-90.0; ANALYZER_POINTS],
            curve_tilt_octaves: [0.0; MANUAL_MASK_POINTS],
            base_curve_cache: [0.0; MANUAL_MASK_POINTS],
            manual_curve_cache: [0.0; MANUAL_MASK_POINTS],
            voices: [VoiceState::EMPTY; MAX_VOICES],
            particles: particle_adapter::Adapter::new(48_000.0),
            mask_voices: [MaskVoice::default(); MAX_MASK_VOICES],
            pinned_note_levels: [0.0; MIDI_NOTES],
            note_levels: [0.0; MIDI_NOTES],
            displayed_note_levels: [0.0; MIDI_NOTES],
            note_level_history: [[0.0; MIDI_NOTES]; OVERLAP_TIMES],
            note_tunings: [0.0; MIDI_NOTES],
            note_timbres: [0.5; MIDI_NOTES],
            midi_expression: MidiExpression::default(),
            channel_sustain: [false; 16],
            motion_phase: 0.0,
            motion_phase_offset: 0.0,
            expression_phase: 0.0,
            hold_was_enabled: false,
            mts_client: mts_client::Client::default(),
            tuning: mts_client::Tuning::default(),
        }
    }
}

impl Plugin for SpectralPlugin {
    const NAME: &'static str = "Oiko Weft";
    const VENDOR: &'static str = "Oiko Audio";
    const URL: &'static str = "https://oikoaudio.com/weft/";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type Editor = EguiEditor<SpectralEditor>;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn validate_state(state: &PluginState) -> Result<(), String> {
        state::validate(state)
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        // Seed geometry from the fixed canvas without compounding user zoom.
        // Hosts may create this editor before restoring state and reuse it on
        // reopen; SpectralEditor::build reconciles the size with restored zoom.
        let interface_scale = closest_ui_scale(self.params.ui_scale.get());
        self.params.ui_scale.set(interface_scale);

        // On macOS, keep egui's own zoom at 1.0 and scale Weft's fixed canvas in
        // the editor instead. This avoids a Retina-specific mismatch between the
        // CLAP host window size and egui-baseview's zoomed backing size.
        self.editor_state =
            oiko_plugin::editor_state(egui::vec2(EDITOR_WIDTH, EDITOR_HEIGHT), interface_scale);
        create_egui_editor(
            self.editor_state.clone(),
            RepaintNotifier::new(),
            EguiNiceSettings::new().with_tile(Self::NAME),
            SpectralEditor::new(self.params.clone(), self.analysis_display.clone()),
        )
    }

    fn activate(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.analysis_display.set_sample_rate(self.sample_rate);
        for (index, octaves) in self.curve_tilt_octaves.iter_mut().enumerate() {
            let frequency =
                (index as f32 / (MANUAL_MASK_POINTS - 1) as f32 * self.sample_rate * 0.5).max(20.0);
            *octaves = (frequency / 1000.0).log2();
        }
        let channels = audio_io_layout
            .main_output_channels
            .map(NonZeroU32::get)
            .unwrap_or(0) as usize;
        self.num_channels = channels.max(1);
        self.stft = util::StftHelper::new(self.num_channels, MAX_FFT_SIZE, 0);

        let mut planner = RealFftPlanner::new();
        self.plans = Some(
            [
                ROUGH_FFT_SIZE,
                COARSE_FFT_SIZE,
                MIN_FFT_SIZE,
                DEFAULT_FFT_SIZE,
                MAX_FFT_SIZE,
            ]
            .map(|size| {
                let mut window = vec![0.0; size];
                let mut smooth_window = vec![0.0; size];
                util::window::hann_in_place(&mut window);
                util::window::blackman_in_place(&mut smooth_window);
                let forward = planner.plan_fft_forward(size);
                let inverse = planner.plan_fft_inverse(size);
                FftPlan {
                    particle_mask: spectral_dsp::particles::Mask::new(size / 2 + 1),
                    particle_gains: vec![1.0; size / 2 + 1],
                    prepared: PreparedSpectrum::new(
                        self.sample_rate,
                        window,
                        smooth_window,
                        OVERLAP_TIMES,
                    ),
                    forward_scratch: forward.make_scratch_vec(),
                    inverse_scratch: inverse.make_scratch_vec(),
                    forward,
                    inverse,
                }
            }),
        );

        self.particles = particle_adapter::Adapter::new(self.sample_rate);
        self.quality = self.params.quality.value();
        self.resize_for_fft(self.quality.size());
        context.set_latency_samples(self.stft.latency_samples());
        self.clear_notes();
        true
    }

    fn reset(&mut self) {
        self.stft.set_block_size(self.quality.size());
        self.hop_position = 0;
        self.motion_phase = 0.0;
        self.motion_phase_offset = self.params.motion_phase_percent.value() * 0.01;
        self.expression_phase = 0.0;
        self.clear_notes();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.tuning = self.mts_client.read();
        let tuning_name = if self.tuning.active {
            self.mts_client.name_bytes()
        } else {
            [0; 256]
        };
        self.analysis_display
            .tuning_snapshot
            .publish(&self.tuning, &tuning_name);
        self.capture_held_notes_when_hold_starts();
        let (tempo_bpm, transport_position_beats) = {
            let transport = context.transport();
            (
                transport
                    .tempo
                    .filter(|v| v.is_finite())
                    .unwrap_or(self.particles.last_tempo as f64)
                    .clamp(1.0, 999.0) as f32,
                transport.pos_beats,
            )
        };
        self.analysis_display.store_tempo(tempo_bpm);
        self.particles.last_tempo = tempo_bpm;
        let quality = self.params.quality.value();
        if quality != self.quality {
            self.quality = quality;
            self.resize_for_fft(quality.size());
            context.set_latency_samples(self.stft.latency_samples());
        }

        self.configure_particles(
            tempo_bpm,
            context.transport().playing || transport_position_beats.is_none(),
        );
        self.particles.transport(
            transport_position_beats,
            context.transport().playing,
            buffer.samples(),
            self.sample_rate,
            self.params.motion_sync.value(),
        );

        let samples = buffer.samples();
        let hop = self.quality.size() / OVERLAP_TIMES;
        let mut next_event = context.next_event();
        let mut offset = 0;
        loop {
            // Apply all same-sample events before generating any samples at that position.
            while next_event.is_some_and(|event| event.timing() as usize <= offset) {
                let event = next_event.take().unwrap();
                if expression_address(event).is_some() {
                    self.queue_expression(event);
                } else {
                    self.consume_note_event(event);
                    self.flush_terminated(context, offset as u32);
                }
                next_event = context.next_event();
            }
            for index in 0..self.expression_event_count {
                if let Some(event) = self.expression_events[index].take() {
                    self.consume_note_event(event);
                }
            }
            self.expression_event_count = 0;
            self.update_particle_sources();
            if offset == samples {
                break;
            }
            let next_time =
                next_event.map_or(samples, |event| (event.timing() as usize).min(samples));
            let end = next_time
                .min(offset + hop - self.hop_position)
                .max(offset + 1);
            let elapsed = end - offset;
            self.particles.engine.advance(elapsed);
            self.advance_expression_time(elapsed);
            self.flush_terminated(context, end.saturating_sub(1) as u32);
            self.midi_expression.resolve(
                &mut self.voices,
                self.params.pitch_bend_range.value() as f32,
            );
            // Frame reference: end of the accumulated hop, independent of host partitions.
            let frame_position = transport_position_beats.map(|position| {
                position
                    + if context.transport().playing {
                        end as f64 / self.sample_rate as f64 * tempo_bpm as f64 / 60.0
                    } else {
                        0.0
                    }
            });
            let mut slice = AudioSlice {
                channels: buffer.as_slice(),
                start: offset,
                len: elapsed,
            };
            self.render_audio(&mut slice, tempo_bpm, frame_position);
            self.hop_position = (self.hop_position + elapsed) % hop;
            offset = end;
        }

        let peak = buffer
            .as_slice()
            .iter()
            .flat_map(|channel| channel.iter())
            .filter(|sample| sample.is_finite())
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        self.analysis_display.store_output_peak(peak);
        ProcessStatus::Normal
    }
}

impl SpectralPlugin {
    fn render_audio(
        &mut self,
        buffer: &mut AudioSlice<'_, '_>,
        tempo_bpm: f32,
        transport_position_beats: Option<f64>,
    ) {
        let fft_size = self.quality.size();
        let quality = self.quality;
        let tuning = self.tuning;
        let plan =
            &mut self.plans.as_mut().expect("FFT plans created on activate")[quality.plan_index()];
        let sample_rate = self.sample_rate;
        let window = &plan.prepared.window;
        let synthesis_window = &plan.prepared.synthesis_window;
        let window_coherent_gain = plan.prepared.window_coherent_gain;
        let smooth_analysis_window = &plan.prepared.smooth_analysis_window;
        let smooth_synthesis_window = &plan.prepared.smooth_synthesis_window;
        let smooth_window_coherent_gain = plan.prepared.smooth_window_coherent_gain;
        let fft_buffer = &mut self.fft_buffer;
        let mask = &mut self.mask;
        let mask_right = &mut self.mask_right;
        let target_mask_right = &mut self.target_mask_right;
        let softened_mask_right = &mut self.softened_mask_right;
        let target_mask = &mut self.target_mask;
        let softened_mask = &mut self.softened_mask;
        let mask_workspace = &plan.prepared.mask_workspace;
        let curve_tilt_octaves = &self.curve_tilt_octaves;
        let base_curve = &mut self.base_curve_cache;
        let manual_curve = &mut self.manual_curve_cache;
        let voices = &mut self.voices;
        let particles = &mut self.particles;
        let mask_voices = &mut self.mask_voices;
        let pinned_note_levels = &mut self.pinned_note_levels;
        let note_levels = &mut self.note_levels;
        let displayed_note_levels = &mut self.displayed_note_levels;
        let note_level_history = &mut self.note_level_history;
        let note_tunings = &mut self.note_tunings;
        let note_timbres = &mut self.note_timbres;
        let analysis_display = &self.analysis_display;
        let analyzer_accumulator = &mut self.analyzer_accumulator;
        let analyzer_points_db = &mut self.analyzer_points_db;
        let capture_accumulator = &mut self.capture_accumulator;
        let num_channels = self.num_channels;
        let params = &self.params;
        let motion_phase = &mut self.motion_phase;
        let motion_phase_offset = &mut self.motion_phase_offset;
        let expression_phase = &mut self.expression_phase;
        let frame_samples = (fft_size / OVERLAP_TIMES) as u32;
        let frame_seconds = frame_samples as f32 / sample_rate;
        let transform_gain = (fft_size as f32).sqrt().recip();
        let mut frame_output_gain = db_to_gain(params.output_gain_db.value());
        let mut frame_smooth_enabled = params.smooth_spectral.value();

        self.stft
            .process_overlap_add(buffer, OVERLAP_TIMES, |channel_index, real_buffer| {
                if channel_index == 0 {
                    frame_smooth_enabled = params.smooth_spectral.value();
                    analyzer_accumulator.fill(0.0);
                    frame_output_gain = db_to_gain(params.output_gain_db.smoothed.previous_value());
                    advance_pinned_notes(
                        pinned_note_levels,
                        &params.pinned_notes,
                        frame_seconds,
                        params.note_attack_ms.value(),
                        params.note_release_ms.value(),
                    );
                    *expression_phase =
                        (*expression_phase + VIBRATO_RATE_HZ * frame_seconds).rem_euclid(1.0);
                    prepare_mask_voices(
                        voices,
                        mask_voices,
                        params
                            .velocity_sensitivity_percent
                            .smoothed
                            .previous_value()
                            * 0.01,
                        *expression_phase,
                        note_levels,
                        note_tunings,
                        note_timbres,
                        pinned_note_levels,
                        &tuning,
                    );
                    update_displayed_note_levels(
                        note_level_history,
                        note_levels,
                        displayed_note_levels,
                    );
                    params.curve.copy_to(base_curve);
                    transform_curve(
                        base_curve,
                        manual_curve,
                        curve_tilt_octaves,
                        params.curve_depth_percent.smoothed.previous_value(),
                        params.curve_tilt_db_per_octave.smoothed.previous_value(),
                        params.curve_shift_semitones.smoothed.previous_value(),
                    );
                    let direction = params.motion_direction.value();
                    let sync = params.motion_sync.value();
                    let division = params.motion_rate_division.value();
                    let requested_motion_rate_hz = if sync {
                        division.rate_hz(tempo_bpm)
                    } else {
                        params.motion_rate_hz.value()
                    };
                    let effective_motion_rate_hz =
                        effective_motion_rate_hz(requested_motion_rate_hz, sample_rate, fft_size);
                    let raw_phase = if sync {
                        if let Some(position_beats) = transport_position_beats {
                            let rate_ratio =
                                effective_motion_rate_hz / requested_motion_rate_hz.max(0.01);
                            ((position_beats / division.beats() as f64) * rate_ratio as f64)
                                .rem_euclid(1.0) as f32
                        } else {
                            *motion_phase = (*motion_phase
                                + effective_motion_rate_hz * frame_seconds)
                                .rem_euclid(1.0);
                            *motion_phase
                        }
                    } else {
                        *motion_phase = (*motion_phase + effective_motion_rate_hz * frame_seconds)
                            .rem_euclid(1.0);
                        *motion_phase
                    };
                    *motion_phase_offset = smooth_circular_phase(
                        *motion_phase_offset,
                        params.motion_phase_percent.value() * 0.01,
                        frame_seconds,
                        0.05,
                    );
                    let displayed_phase = (directed_motion_phase(raw_phase, direction)
                        + *motion_phase_offset)
                        .rem_euclid(1.0);
                    let note_depth_db = params.note_depth_db.smoothed.previous_value();
                    let motion_depth_db = params.motion_depth_db.smoothed.previous_value();
                    let motion_config = MotionConfig {
                        shape: params.motion_shape.value().into(),
                        depth_db: motion_depth_db,
                        phase: displayed_phase,
                        size_octaves: params.motion_size_octaves.value(),
                    };
                    let separate_motion = particles.begin_frame(
                        motion_config,
                        sample_rate / fft_size as f32,
                        target_mask.len(),
                    );
                    let mask_config = MaskConfig {
                        sample_rate,
                        fft_size,
                        note_depth_db,
                        note_layer_enabled: true,
                        width_cents: params.width_cents.value(),
                        partials: params.partials.value() as usize,
                        harmonic_rolloff_db_per_octave: params.harmonic_rolloff_db.value(),
                        motion: MotionConfig {
                            depth_db: if separate_motion {
                                0.0
                            } else {
                                motion_depth_db
                            },
                            ..motion_config
                        },
                    };
                    if num_channels == 2 {
                        spectral_dsp::build_stereo_masks_with_voices_precomputed(
                            target_mask,
                            target_mask_right,
                            manual_curve,
                            mask_voices,
                            mask_config,
                            mask_workspace,
                        );
                    } else {
                        build_mask_with_voices_precomputed(
                            target_mask,
                            manual_curve,
                            mask_voices,
                            mask_config,
                            mask_workspace,
                        );
                    }
                    let particle_frame = particles
                        .engine
                        .frame(sample_rate / fft_size as f32, frame_seconds);
                    plan.particle_mask.gains(
                        &particle_frame,
                        motion_depth_db,
                        mask_workspace,
                        &mut plan.particle_gains,
                    );
                    particles.apply(
                        motion_config,
                        frame_seconds,
                        sample_rate / fft_size as f32,
                        &plan.particle_gains,
                        target_mask,
                        if num_channels == 2 {
                            Some(target_mask_right)
                        } else {
                            None
                        },
                    );
                    particles.publish(
                        analysis_display,
                        frame_samples as usize,
                        target_mask.len(),
                        sample_rate / fft_size as f32,
                    );
                    if frame_smooth_enabled {
                        soften_spectral_edges(target_mask, softened_mask);
                        smooth_mask_in_db(mask, softened_mask, frame_seconds);
                        if num_channels == 2 {
                            soften_spectral_edges(target_mask_right, softened_mask_right);
                            smooth_mask_in_db(mask_right, softened_mask_right, frame_seconds);
                        }
                    } else {
                        mask.copy_from_slice(target_mask);
                        if num_channels == 2 {
                            mask_right.copy_from_slice(target_mask_right);
                        }
                    }
                    analysis_display.store_notes(displayed_note_levels, note_tunings, note_timbres);
                    analysis_display.store_motion_phase(displayed_phase);
                }

                let (analysis_window, analysis_gain, coherent_gain) = if frame_smooth_enabled {
                    (
                        smooth_analysis_window.as_slice(),
                        transform_gain,
                        smooth_window_coherent_gain,
                    )
                } else {
                    (window.as_slice(), transform_gain, window_coherent_gain)
                };
                for (sample, window_sample) in real_buffer.iter_mut().zip(analysis_window.iter()) {
                    *sample *= window_sample * analysis_gain;
                }
                plan.forward
                    .process_with_scratch(real_buffer, fft_buffer, &mut plan.forward_scratch)
                    .expect("preallocated forward FFT");
                for (accumulator, bin) in analyzer_accumulator.iter_mut().zip(fft_buffer.iter()) {
                    *accumulator += bin.norm();
                }
                if channel_index + 1 == num_channels {
                    publish_analyzer(
                        analyzer_accumulator,
                        analyzer_points_db,
                        analysis_display,
                        sample_rate,
                        fft_size,
                        num_channels,
                        analysis_gain,
                        coherent_gain,
                    );
                    capture_accumulator.push(
                        analyzer_points_db,
                        frame_seconds,
                        &analysis_display.capture,
                    );
                }
                for (bin, gain) in fft_buffer.iter_mut().zip(if channel_index == 0 {
                    mask.iter()
                } else {
                    mask_right.iter()
                }) {
                    *bin *= *gain;
                }
                plan.inverse
                    .process_with_scratch(fft_buffer, real_buffer, &mut plan.inverse_scratch)
                    .expect("preallocated inverse FFT");

                let (synthesis_window, synthesis_gain) = if frame_smooth_enabled {
                    (smooth_synthesis_window.as_slice(), transform_gain)
                } else {
                    (synthesis_window.as_slice(), transform_gain)
                };
                for (sample, window_sample) in real_buffer.iter_mut().zip(synthesis_window.iter()) {
                    *sample *= window_sample * synthesis_gain * frame_output_gain;
                }
            });
    }

    fn advance_expression_time(&mut self, samples: usize) {
        let elapsed = samples as f32 / self.sample_rate;
        for voice in &mut self.voices {
            let previous = *voice;
            advance_voices(
                std::slice::from_mut(voice),
                elapsed,
                self.params.note_attack_ms.value(),
                self.params.note_release_ms.value(),
            );
            if previous.occupied && !voice.occupied {
                self.terminated[self.terminated_count] = previous;
                self.terminated_count += 1;
            }
        }
        let p = &self.params;
        p.output_gain_db.smoothed.next_step(samples as u32);
        p.velocity_sensitivity_percent
            .smoothed
            .next_step(samples as u32);
        p.curve_depth_percent.smoothed.next_step(samples as u32);
        p.curve_tilt_db_per_octave
            .smoothed
            .next_step(samples as u32);
        p.curve_shift_semitones.smoothed.next_step(samples as u32);
        p.note_depth_db.smoothed.next_step(samples as u32);
        p.motion_depth_db.smoothed.next_step(samples as u32);
    }

    fn flush_terminated(&mut self, context: &mut impl ProcessContext<Self>, timing: u32) {
        for voice in &self.terminated[..self.terminated_count] {
            let _ = context.try_send_event(NoteEvent::VoiceTerminated {
                timing,
                voice_id: voice.voice_id.map_or(VoiceID::Wildcard, VoiceID::ID),
                channel: Channel::Number(voice.channel),
                key: Key::Number(voice.note),
            });
        }
        self.terminated_count = 0;
    }

    fn capture_held_notes_when_hold_starts(&mut self) {
        let hold_enabled = self.params.pinned_notes.capture_incoming();
        if hold_enabled && !self.hold_was_enabled {
            for voice in &self.voices {
                if voice.occupied && voice.held {
                    self.params.pinned_notes.capture(voice.note as usize);
                }
            }
        }
        self.hold_was_enabled = hold_enabled;
    }

    fn resize_for_fft(&mut self, fft_size: usize) {
        self.stft.set_block_size(fft_size);
        self.hop_position = 0;
        self.mask_right.resize(fft_size / 2 + 1, 1.0);
        self.target_mask_right.resize(fft_size / 2 + 1, 1.0);
        self.softened_mask_right.resize(fft_size / 2 + 1, 1.0);
        self.fft_buffer
            .resize(fft_size / 2 + 1, Complex32::default());
        self.mask.resize(fft_size / 2 + 1, 1.0);
        self.target_mask.resize(fft_size / 2 + 1, 1.0);
        self.softened_mask.resize(fft_size / 2 + 1, 1.0);
        self.analyzer_accumulator.resize(fft_size / 2 + 1, 0.0);
        self.note_level_history.fill([0.0; MIDI_NOTES]);
        self.displayed_note_levels.fill(0.0);
    }

    fn queue_expression(&mut self, event: NoteEvent<()>) {
        // Keep the latest value per address/type, in arrival order relative to
        // wildcard and more specific updates. The bound matches voice capacity
        // times the seven standardized dimensions. On overflow drop the oldest.
        let duplicate = self.expression_events[..self.expression_event_count]
            .iter()
            .position(|previous| {
                previous.is_some_and(|previous| {
                    expression_address(previous) == expression_address(event)
                        && std::mem::discriminant(&previous) == std::mem::discriminant(&event)
                })
            });
        let remove = duplicate
            .or_else(|| (self.expression_event_count == self.expression_events.len()).then_some(0));
        if let Some(index) = remove {
            self.expression_events
                .copy_within(index + 1..self.expression_event_count, index);
            self.expression_event_count -= 1;
        }
        self.expression_events[self.expression_event_count] = Some(event);
        self.expression_event_count += 1;
    }

    fn consume_note_event(&mut self, event: NoteEvent<()>) {
        match event {
            NoteEvent::NoteOn {
                voice_id,
                channel: Channel::Number(channel),
                key: Key::Number(note),
                velocity,
                ..
            } if channel < 16 && note < 128 && velocity.is_finite() => {
                self.start_voice(voice_id.id(), channel, note, velocity);
            }
            NoteEvent::NoteOff {
                voice_id,
                channel,
                key,
                ..
            } => self.release_voice(voice_id, channel, key),
            NoteEvent::Choke {
                voice_id,
                channel,
                key,
                ..
            } => self.choke_voice(voice_id, channel, key),
            NoteEvent::PolyTuning {
                voice_id,
                channel,
                key,
                tuning,
                ..
            } => {
                for voice in &mut self.voices {
                    if voice_matches(voice, voice_id, channel, key) && tuning.is_finite() {
                        voice.tuning_semitones = tuning.clamp(-120.0, 120.0);
                    }
                }
            }
            NoteEvent::PolyPressure {
                voice_id,
                channel,
                key,
                pressure,
                ..
            } => {
                for voice in &mut self.voices {
                    if voice_matches(voice, voice_id, channel, key) && pressure.is_finite() {
                        voice.native_pressure = Some(pressure.clamp(0.0, 1.0));
                    }
                }
            }
            NoteEvent::PolyVolume {
                voice_id,
                channel,
                key,
                gain,
                ..
            } => {
                for voice in &mut self.voices {
                    if voice_matches(voice, voice_id, channel, key) && gain.is_finite() {
                        voice.volume_gain = gain.clamp(0.0, 4.0);
                    }
                }
            }
            NoteEvent::PolyVibrato {
                voice_id,
                channel,
                key,
                vibrato,
                ..
            } => {
                for voice in &mut self.voices {
                    if voice_matches(voice, voice_id, channel, key) && vibrato.is_finite() {
                        voice.vibrato = vibrato.clamp(0.0, 1.0);
                    }
                }
            }
            NoteEvent::PolyBrightness {
                voice_id,
                channel,
                key,
                brightness,
                ..
            } => {
                for voice in &mut self.voices {
                    if voice_matches(voice, voice_id, channel, key) && brightness.is_finite() {
                        voice.native_timbre = Some(brightness.clamp(0.0, 1.0));
                    }
                }
            }
            NoteEvent::PolyExpression {
                voice_id,
                channel,
                key,
                expression,
                ..
            } => {
                for voice in &mut self.voices {
                    if voice_matches(voice, voice_id, channel, key) && expression.is_finite() {
                        voice.expression_amount = expression.clamp(0.0, 1.0);
                    }
                }
            }
            NoteEvent::PolyPan {
                voice_id,
                channel,
                key,
                pan,
                ..
            } => {
                if pan.is_finite() {
                    for voice in &mut self.voices {
                        if voice_matches(voice, voice_id, channel, key) {
                            voice.pan = pan.clamp(-1.0, 1.0);
                        }
                    }
                }
            }
            NoteEvent::MidiPitchBend { channel, value, .. } => {
                self.midi_expression.pitch_bend(channel, value)
            }
            NoteEvent::MidiChannelPressure {
                channel, pressure, ..
            } => self.midi_expression.pressure(channel, pressure),
            NoteEvent::MidiCC {
                channel, cc, value, ..
            } if channel < 16 && value.is_finite() => {
                let old_zones: [Option<u8>; 16] =
                    std::array::from_fn(|c| self.midi_expression.zone_for(c as u8));
                let member = self.midi_expression.master_for(channel).is_some();
                match cc {
                    64 if !member => self.set_sustain(channel, value >= 0.5),
                    120 | 123 if !member => {
                        for affected in 0..16 {
                            if affected == channel
                                || self.midi_expression.master_for(affected) == Some(channel)
                            {
                                if cc == 120 {
                                    self.choke_voice(
                                        VoiceID::Wildcard,
                                        Channel::Number(affected),
                                        Key::Wildcard,
                                    );
                                } else {
                                    self.release_voice(
                                        VoiceID::Wildcard,
                                        Channel::Number(affected),
                                        Key::Wildcard,
                                    );
                                }
                            }
                        }
                    }
                    121 if !member => {
                        self.set_sustain(channel, false);
                        for affected in 0..16 {
                            if affected == channel
                                || self.midi_expression.master_for(affected) == Some(channel)
                            {
                                self.midi_expression.cc(affected, cc, value);
                            }
                        }
                    }
                    64 | 120 | 121 | 123 => {}
                    _ => self.midi_expression.cc(channel, cc, value),
                }
                for (affected, old_zone) in old_zones.into_iter().enumerate() {
                    if old_zone != self.midi_expression.zone_for(affected as u8) {
                        self.choke_voice(
                            VoiceID::Wildcard,
                            Channel::Number(affected as u8),
                            Key::Wildcard,
                        );
                        self.channel_sustain[affected] = false;
                    }
                }
            }
            _ => {}
        }
    }

    fn start_voice(&mut self, voice_id: Option<i32>, channel: u8, note: u8, velocity: f32) {
        if self.params.pinned_notes.capture_incoming() {
            self.params.pinned_notes.capture(note as usize);
        }
        let index = voice_id
            .and_then(|id| {
                self.voices
                    .iter()
                    .position(|voice| voice.occupied && voice.voice_id == Some(id))
            })
            .or_else(|| self.voices.iter().position(|voice| !voice.occupied))
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by(|(_, left), (_, right)| left.level.total_cmp(&right.level))
                    .map(|(index, _)| index)
                    .unwrap_or(0)
            });
        if self.voices[index].occupied {
            self.terminated[self.terminated_count] = self.voices[index];
            self.terminated_count += 1;
        }
        self.particles.engine.retire_source(index);
        self.particles.pending[index] = Some(self.particles.engine.reserve_trigger());
        self.voices[index] = VoiceState {
            occupied: true,
            held: true,
            voice_id,
            channel,
            note,
            velocity: velocity.clamp(0.0, 1.0),
            ..VoiceState::EMPTY
        };
    }

    fn release_voice(&mut self, voice_id: VoiceID, channel: Channel, key: Key) {
        for voice in &mut self.voices {
            if voice.held && voice_matches(voice, voice_id, channel, key) {
                voice.held = false;
                voice.sustained = self.channel_sustain[voice.channel as usize];
            }
        }
    }

    fn set_sustain(&mut self, channel: u8, down: bool) {
        for affected in 0..16 {
            if affected == channel || self.midi_expression.master_for(affected) == Some(channel) {
                self.channel_sustain[affected as usize] = down;
                if !down {
                    for voice in &mut self.voices {
                        if voice.channel == affected {
                            voice.sustained = false;
                        }
                    }
                }
            }
        }
    }

    fn choke_voice(&mut self, voice_id: VoiceID, channel: Channel, key: Key) {
        for (index, voice) in self.voices.iter_mut().enumerate() {
            if voice_matches(voice, voice_id, channel, key) {
                self.terminated[self.terminated_count] = *voice;
                self.terminated_count += 1;
                self.particles.engine.retire_source(index);
                self.particles.pending[index] = None;
                *voice = VoiceState::EMPTY;
            }
        }
    }

    fn clear_notes(&mut self) {
        self.voices.fill(VoiceState::EMPTY);
        self.particles.reset();
        self.mask_voices.fill(MaskVoice::default());
        self.pinned_note_levels.fill(0.0);
        self.note_levels.fill(0.0);
        self.displayed_note_levels.fill(0.0);
        self.note_level_history.fill([0.0; MIDI_NOTES]);
        self.note_tunings.fill(0.0);
        self.note_timbres.fill(0.5);
        self.midi_expression = MidiExpression::default();
        self.terminated_count = 0;
        self.expression_events.fill(None);
        self.expression_event_count = 0;
        self.channel_sustain.fill(false);
        self.analysis_display.clear();
    }
}

fn expression_address(event: NoteEvent<()>) -> Option<(VoiceID, Channel, Key)> {
    match event {
        NoteEvent::PolyTuning {
            voice_id,
            channel,
            key,
            ..
        }
        | NoteEvent::PolyVolume {
            voice_id,
            channel,
            key,
            ..
        }
        | NoteEvent::PolyPan {
            voice_id,
            channel,
            key,
            ..
        }
        | NoteEvent::PolyPressure {
            voice_id,
            channel,
            key,
            ..
        }
        | NoteEvent::PolyBrightness {
            voice_id,
            channel,
            key,
            ..
        }
        | NoteEvent::PolyExpression {
            voice_id,
            channel,
            key,
            ..
        }
        | NoteEvent::PolyVibrato {
            voice_id,
            channel,
            key,
            ..
        } => Some((voice_id, channel, key)),
        _ => None,
    }
}

fn voice_matches(voice: &VoiceState, voice_id: VoiceID, channel: Channel, key: Key) -> bool {
    voice.occupied
        && (channel.is_wildcard() || channel == Channel::Number(voice.channel))
        && (key.is_wildcard() || key == Key::Number(voice.note))
        && (voice_id.is_wildcard() || voice_id.id() == voice.voice_id)
}

fn advance_voices(
    voices: &mut [VoiceState],
    elapsed_seconds: f32,
    attack_ms: f32,
    release_ms: f32,
) {
    for voice in voices {
        if !voice.occupied {
            continue;
        }
        let active = voice.held || voice.sustained;
        let target = if active { 1.0 } else { 0.0 };
        let time_ms = if active { attack_ms } else { release_ms };
        let alpha = if time_ms <= 0.0 {
            1.0
        } else {
            1.0 - (-elapsed_seconds / (time_ms * 0.001)).exp()
        };
        voice.level += (target - voice.level) * alpha;
        if !active && voice.level < 1.0e-6 {
            *voice = VoiceState::EMPTY;
        }
    }
}

fn advance_pinned_notes(
    levels: &mut [f32; MIDI_NOTES],
    pinned: &PinnedNotesState,
    elapsed_seconds: f32,
    attack_ms: f32,
    release_ms: f32,
) {
    for (note, level) in levels.iter_mut().enumerate() {
        let held = pinned.get(note);
        let target = f32::from(held);
        let time_ms = if held { attack_ms } else { release_ms };
        let alpha = if time_ms <= 0.0 {
            1.0
        } else {
            1.0 - (-elapsed_seconds / (time_ms * 0.001)).exp()
        };
        *level += (target - *level) * alpha;
        if !held && *level < 1.0e-6 {
            *level = 0.0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_mask_voices(
    voices: &[VoiceState],
    mask_voices: &mut [MaskVoice],
    velocity_sensitivity: f32,
    expression_phase: f32,
    note_levels: &mut [f32; MIDI_NOTES],
    note_tunings: &mut [f32; MIDI_NOTES],
    note_timbres: &mut [f32; MIDI_NOTES],
    pinned_note_levels: &[f32; MIDI_NOTES],
    mts: &mts_client::Tuning,
) {
    note_levels.fill(0.0);
    note_tunings.fill(0.0);
    note_timbres.fill(0.5);
    let vibrato_wave = (std::f32::consts::TAU * expression_phase).sin();
    let (live_targets, pinned_targets) = mask_voices.split_at_mut(voices.len());
    for (source, target) in voices.iter().zip(live_targets.iter_mut()) {
        if !source.occupied {
            *target = MaskVoice::default();
            continue;
        }
        let tuning = mts.offset(source.note as usize)
            + source.expression.tuning_semitones
            + vibrato_wave * source.vibrato * VIBRATO_RANGE_SEMITONES;
        let velocity_gain = effective_velocity(source.velocity, velocity_sensitivity);
        *target = MaskVoice {
            note: source.note,
            level: if mts.mapped[source.note as usize] {
                source.level
            } else {
                0.0
            },
            tuning_semitones: tuning,
            pressure: source.expression.pressure,
            timbre: source.expression.timbre,
            volume_gain: velocity_gain,
            expression_gain: source.expression.gain,
            pan: source.expression.pan,
        };

        let display_level = target.level
            * source.expression.gain
            * velocity_gain
            * (0.35 + 0.65 * source.expression.pressure.clamp(0.0, 1.0));
        let note = source.note as usize;
        if display_level > note_levels[note] {
            note_levels[note] = display_level;
            note_tunings[note] = tuning;
            note_timbres[note] = source.expression.timbre;
        }
    }
    for (note, (level, target)) in pinned_note_levels
        .iter()
        .zip(pinned_targets.iter_mut())
        .enumerate()
    {
        let velocity_gain = effective_velocity(PINNED_NOTE_VELOCITY, velocity_sensitivity);
        *target = MaskVoice {
            note: note as u8,
            level: if mts.mapped[note] { *level } else { 0.0 },
            tuning_semitones: mts.offset(note),
            volume_gain: velocity_gain,
            ..MaskVoice::default()
        };
        let display_level = target.level * velocity_gain;
        if display_level > note_levels[note] {
            note_levels[note] = display_level;
            note_tunings[note] = mts.offset(note);
            note_timbres[note] = 0.5;
        }
    }
}

fn effective_velocity(velocity: f32, sensitivity: f32) -> f32 {
    1.0 + (velocity.clamp(0.0, 1.0) - 1.0) * sensitivity.clamp(0.0, 1.0)
}

fn update_displayed_note_levels(
    history: &mut [[f32; MIDI_NOTES]; OVERLAP_TIMES],
    current: &[f32; MIDI_NOTES],
    displayed: &mut [f32; MIDI_NOTES],
) {
    history.rotate_right(1);
    history[0].copy_from_slice(current);

    // The midpoint of a four-times-overlapped Hann-squared output hop has
    // these normalized contributions from newest to oldest frame. Mirroring
    // them makes the blue note mask follow the audible overlap/add handover
    // rather than jumping straight to the raw note-envelope target.
    const HANN_MIDPOINT_WEIGHTS: [f32; OVERLAP_TIMES] =
        [0.014_297_74, 0.485_702_25, 0.485_702_25, 0.014_297_74];
    for note in 0..MIDI_NOTES {
        displayed[note] = history
            .iter()
            .zip(HANN_MIDPOINT_WEIGHTS)
            .map(|(frame, weight)| frame[note] * weight)
            .sum();
    }
}

#[allow(clippy::too_many_arguments)]
fn publish_analyzer(
    accumulated_magnitudes: &[f32],
    output_db: &mut [f32; ANALYZER_POINTS],
    display: &AnalysisDisplay,
    sample_rate: f32,
    fft_size: usize,
    num_channels: usize,
    analysis_gain: f32,
    window_coherent_gain: f32,
) {
    let minimum_frequency = spectral_dsp::MIN_DISPLAY_FREQUENCY_HZ;
    let maximum_frequency = display_max_frequency(sample_rate);
    let frequency_ratio = maximum_frequency / minimum_frequency;
    let bin_hz = sample_rate / fft_size as f32;
    // A bin-centered sine has magnitude A*N*coherent_gain/2. Undo that and
    // the analysis-stage transform normalization for either window profile.
    let amplitude_scale = 2.0
        / (fft_size as f32
            * window_coherent_gain.max(1.0e-6)
            * analysis_gain
            * num_channels as f32);

    for (point, output) in output_db.iter_mut().enumerate() {
        let lower_position = (point as f32 - 0.5).max(0.0) / (ANALYZER_POINTS - 1) as f32;
        let upper_position =
            (point as f32 + 0.5).min((ANALYZER_POINTS - 1) as f32) / (ANALYZER_POINTS - 1) as f32;
        let lower_frequency = minimum_frequency * frequency_ratio.powf(lower_position);
        let upper_frequency = minimum_frequency * frequency_ratio.powf(upper_position);
        let first_bin = (lower_frequency / bin_hz).floor().max(1.0) as usize;
        let last_bin = ((upper_frequency / bin_hz).ceil() as usize)
            .min(accumulated_magnitudes.len().saturating_sub(1));
        let magnitude = accumulated_magnitudes[first_bin..=last_bin]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        *output = (20.0
            * (magnitude * amplitude_scale)
                .max(db_to_gain(MANUAL_CURVE_MUTE_DB))
                .log10())
        .clamp(MANUAL_CURVE_MUTE_DB, 6.0);
    }
    display.store_spectrum(output_db);
}

fn triangle_phase(phase: f32) -> f32 {
    1.0 - (phase.rem_euclid(1.0) * 2.0 - 1.0).abs()
}

fn directed_motion_phase(phase: f32, direction: MotionDirection) -> f32 {
    match direction {
        MotionDirection::Forward => phase.rem_euclid(1.0),
        MotionDirection::Reverse => (-phase).rem_euclid(1.0),
        MotionDirection::Alternate => triangle_phase(phase),
    }
}

fn smooth_circular_phase(current: f32, target: f32, seconds: f32, smoothing_seconds: f32) -> f32 {
    let shortest_delta = (target - current + 0.5).rem_euclid(1.0) - 0.5;
    let amount = if smoothing_seconds <= 0.0 {
        1.0
    } else {
        1.0 - (-seconds.max(0.0) / smoothing_seconds).exp()
    };
    (current + shortest_delta * amount).rem_euclid(1.0)
}

impl ClapPlugin for SpectralPlugin {
    const CLAP_SUPPORTS_MPE: bool = true;
    const CLAP_POLY_MODULATION_CONFIG: Option<PolyModulationConfig> = Some(PolyModulationConfig {
        max_voice_capacity: MAX_VOICES as u32,
        supports_overlapping_voices: true,
    });
    const CLAP_ID: &'static str = "com.oikoaudio.weft";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Drawable and MIDI-playable spectral mask");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Filter,
        ClapFeature::PhaseVocoder,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Custom("oiko:spectral-mask"),
    ];
}

impl Vst3Plugin for SpectralPlugin {
    // UUID 6fd2d5de-42a3-4ea4-a2fc-89789764408e. Keep stable once released.
    const VST3_CLASS_ID: [u8; 16] = [
        0x6f, 0xd2, 0xd5, 0xde, 0x42, 0xa3, 0x4e, 0xa4, 0xa2, 0xfc, 0x89, 0x78, 0x97, 0x64, 0x40,
        0x8e,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Filter];
}

nice_export_clap!(SpectralPlugin);
nice_export_vst3!(SpectralPlugin);

#[cfg(test)]
mod tests;
