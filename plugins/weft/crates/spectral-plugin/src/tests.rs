use crate::curve::transformed_curve_db;
use crate::parameters::MAX_FREE_MOTION_RATE_HZ;
use crate::parameters::MotionDirection;
use crate::parameters::SpectralParams;
use crate::state::CurveState;
use crate::state::PinnedNotesState;
use crate::state::SpectrumRangeState;
use crate::state::UiScaleState;
use nice_plug::params::persist::PersistentField;
#[test]
fn resolution_changes_do_not_allocate_or_deallocate() {
    let mut plugin = super::SpectralPlugin::default();
    plugin.resize_for_fft(super::ROUGH_FFT_SIZE);
    nice_assert_no_alloc::assert_no_alloc(|| {
        for size in [16384, 2048, 8192, 1024, 4096, 16384] {
            plugin.resize_for_fft(size);
        }
    });
}
use super::*;

fn unity_stft_max_error(fft_size: usize, smooth: bool) -> f32 {
    let overlap = OVERLAP_TIMES;
    let hop = fft_size / overlap;
    let length = fft_size * 5;
    let input = (0..length)
        .map(|index| {
            let time = index as f32 / 48_000.0;
            let off_bin_tone = (std::f32::consts::TAU * 997.3 * time).sin() * 0.37;
            let second_tone = (std::f32::consts::TAU * 7_113.7 * time).sin() * 0.19;
            let noise = (((index as u32)
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                >> 8) as f32
                / 16_777_215.0
                - 0.5)
                * 0.1;
            off_bin_tone + second_tone + noise
        })
        .collect::<Vec<_>>();
    let mut output = vec![0.0_f32; length + fft_size];
    let mut analysis_window = vec![0.0; fft_size];
    let mut synthesis_window = vec![0.0; fft_size];
    let (analysis_gain, synthesis_gain) = if smooth {
        util::window::blackman_in_place(&mut analysis_window);
        build_dual_synthesis_window(&analysis_window, &mut synthesis_window, overlap);
        let gain = (fft_size as f32).sqrt().recip();
        (gain, gain)
    } else {
        util::window::hann_in_place(&mut analysis_window);
        build_dual_synthesis_window(&analysis_window, &mut synthesis_window, overlap);
        let gain = (fft_size as f32).sqrt().recip();
        (gain, gain)
    };
    let mut planner = RealFftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(fft_size);
    let inverse = planner.plan_fft_inverse(fft_size);
    let mut real = vec![0.0; fft_size];
    let mut complex = vec![Complex32::default(); fft_size / 2 + 1];
    for start in (0..length).step_by(hop) {
        for index in 0..fft_size {
            real[index] = input.get(start + index).copied().unwrap_or(0.0)
                * analysis_window[index]
                * analysis_gain;
        }
        forward.process(&mut real, &mut complex).unwrap();
        inverse.process(&mut complex, &mut real).unwrap();
        for index in 0..fft_size {
            output[start + index] += real[index] * synthesis_window[index] * synthesis_gain;
        }
    }
    input[fft_size..length - fft_size]
        .iter()
        .zip(&output[fft_size..length - fft_size])
        .map(|(input, output)| (input - output).abs())
        .fold(0.0_f32, f32::max)
}

#[test]
fn clap_identity_is_stable() {
    assert_eq!(
        <SpectralPlugin as ClapPlugin>::CLAP_ID,
        "com.oikoaudio.weft"
    );
}

#[test]
fn unity_stft_reconstructs_off_bin_material_at_every_resolution() {
    for fft_size in [
        ROUGH_FFT_SIZE,
        COARSE_FFT_SIZE,
        MIN_FFT_SIZE,
        DEFAULT_FFT_SIZE,
        MAX_FFT_SIZE,
    ] {
        let current_error = unity_stft_max_error(fft_size, false);
        let smooth_error = unity_stft_max_error(fft_size, true);
        assert!(
            current_error < 2.0e-5,
            "current {fft_size}-point path error was {current_error}"
        );
        assert!(
            smooth_error < 2.0e-5,
            "smooth {fft_size}-point path error was {smooth_error}"
        );
    }
}

#[test]
fn smooth_window_has_exact_overlap_product() {
    for fft_size in [1024, 2048, 4096, 8192, 16_384] {
        let mut analysis = vec![0.0; fft_size];
        let mut synthesis = vec![0.0; fft_size];
        util::window::blackman_in_place(&mut analysis);
        build_dual_synthesis_window(&analysis, &mut synthesis, OVERLAP_TIMES);
        let hop = fft_size / OVERLAP_TIMES;
        for index in 0..hop {
            let overlap_sum = (0..OVERLAP_TIMES)
                .map(|offset| {
                    let position = index + offset * hop;
                    analysis[position] * synthesis[position]
                })
                .sum::<f32>();
            assert!((overlap_sum - 1.0).abs() < 2.0e-6);
        }
    }
}

#[test]
fn motion_rate_ceiling_preserves_rough_and_tapers_with_resolution() {
    let ceilings =
        [1024, 2048, 4096, 8192, 16_384].map(|fft_size| maximum_motion_rate_hz(48_000.0, fft_size));
    assert_eq!(ceilings[0], MAX_FREE_MOTION_RATE_HZ);
    assert!(ceilings.windows(2).all(|pair| pair[1] < pair[0]));
    assert!((ceilings[3] - 7.5).abs() < 1.0e-6);
    assert!((ceilings[4] - 3.75).abs() < 1.0e-6);
}

#[test]
fn spectral_softness_rounds_a_single_bin_in_db() {
    let source = [1.0, 1.0, db_to_gain(-60.0), 1.0, 1.0];
    let mut softened = [0.0; 5];
    soften_spectral_edges(&source, &mut softened);
    assert!((20.0 * softened[2].log10() + 36.0).abs() < 1.0e-4);
    assert!((20.0 * softened[1].log10() + 12.0).abs() < 1.0e-4);
    assert_eq!(softened[0], 1.0);
    assert_eq!(softened[4], 1.0);
}

#[test]
fn curve_state_round_trips_through_persistence() {
    let curve = CurveState::default();
    curve.set(12, -18.5);
    curve.set_transformed(13, 9.25);
    curve.set(14, -100.0);
    let snapshot = curve.map(Clone::clone);
    let restored = CurveState::default();
    PersistentField::set(&restored, snapshot);
    assert!((restored.get(12) + 18.5).abs() < 1.0e-6);
    assert!((restored.get(13) - 9.25).abs() < 1.0e-6);
    assert!((restored.get(14) + 100.0).abs() < 1.0e-6);
}

#[test]
fn interface_scale_round_trips_and_stays_within_supported_bounds() {
    let scale = UiScaleState::default();
    scale.set(1.75);
    let snapshot = PersistentField::map(&scale, |value| *value);
    let restored = UiScaleState::default();
    PersistentField::set(&restored, snapshot);
    assert!((restored.get() - 1.75).abs() < 1.0e-6);

    restored.set(3.0);
    assert!((restored.get() - 2.0).abs() < 1.0e-6);
}

#[test]
fn spectrum_range_round_trips_and_snaps_to_supported_values() {
    let range = SpectrumRangeState::default();
    assert_eq!(range.get(), 60.0);
    range.set(90.0);
    let snapshot = PersistentField::map(&range, |value| *value);
    let restored = SpectrumRangeState::default();
    PersistentField::set(&restored, snapshot);
    assert_eq!(restored.get(), 90.0);

    restored.set(44.0);
    assert_eq!(restored.get(), 30.0);

    restored.set(144.0);
    assert_eq!(restored.get(), 144.0);
}

#[test]
fn parameter_ids_are_stable_and_unique() {
    let ids: Vec<_> = SpectralParams::default()
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids.len(), 22);
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len());
}

#[test]
fn motion_rate_normalization_preserves_the_host_parameter_mapping() {
    let params = SpectralParams::default();
    assert!((params.motion_rate_hz.preview_normalized(4.0) - 0.5).abs() < 1.0e-6);
    assert!((params.motion_rate_hz.preview_plain(1.0) - MAX_FREE_MOTION_RATE_HZ).abs() < 1.0e-6);
}

#[test]
fn note_depth_devotes_half_its_travel_to_the_first_20_db() {
    let params = SpectralParams::default();
    assert!((params.note_depth_db.preview_normalized(20.0) - 0.5).abs() < 1.0e-6);
    assert!((params.note_depth_db.preview_plain(0.5) - 20.0).abs() < 1.0e-6);
}

#[test]
fn velocity_sensitivity_blends_from_full_to_input_velocity() {
    assert!((effective_velocity(0.25, 0.0) - 1.0).abs() < 1.0e-6);
    assert!((effective_velocity(0.25, 0.5) - 0.625).abs() < 1.0e-6);
    assert!((effective_velocity(0.25, 1.0) - 0.25).abs() < 1.0e-6);
    assert!((effective_velocity(PINNED_NOTE_VELOCITY, 1.0) - 0.5).abs() < 1.0e-6);
    assert_eq!(
        SpectralParams::default()
            .velocity_sensitivity_percent
            .value(),
        0.0
    );
}

#[test]
fn motion_depth_devotes_two_thirds_of_its_travel_to_the_first_30_db() {
    let params = SpectralParams::default();
    assert!((params.motion_depth_db.preview_normalized(30.0) - 2.0 / 3.0).abs() < 1.0e-6);
    assert!((params.motion_depth_db.preview_plain(2.0 / 3.0) - 30.0).abs() < 1.0e-5);
    assert!((params.motion_depth_db.preview_plain(1.0) - 60.0).abs() < 1.0e-6);
}

#[test]
fn motion_size_reaches_eight_octaves_without_sacrificing_narrow_control() {
    let params = SpectralParams::default();
    let two_octaves = params.motion_size_octaves.preview_normalized(2.0);
    assert!((0.59..=0.61).contains(&two_octaves));
    assert!((params.motion_size_octaves.preview_plain(1.0) - 8.0).abs() < 1.0e-6);
}

#[test]
fn alternate_motion_phase_reverses_without_a_jump() {
    assert!((triangle_phase(0.0) - 0.0).abs() < 1.0e-6);
    assert!((triangle_phase(0.25) - 0.5).abs() < 1.0e-6);
    assert!((triangle_phase(0.5) - 1.0).abs() < 1.0e-6);
    assert!((triangle_phase(0.75) - 0.5).abs() < 1.0e-6);
}

#[test]
fn phase_offset_smoothing_uses_the_short_path_across_wrap() {
    let smoothed = smooth_circular_phase(0.99, 0.01, 0.01, 0.05);
    assert!(smoothed > 0.99 || smoothed < 0.01);
    let reverse = directed_motion_phase(0.25, MotionDirection::Reverse);
    assert!((reverse - 0.75).abs() < 1.0e-6);
}

#[test]
fn analyzer_maps_nonzero_fft_energy_to_visible_points() {
    let mut magnitudes = vec![0.0; DEFAULT_FFT_SIZE / 2 + 1];
    let bin = (440.0 * DEFAULT_FFT_SIZE as f32 / 48_000.0).round() as usize;
    magnitudes[bin] = 100.0;
    let mut output = [-90.0; ANALYZER_POINTS];
    let display = AnalysisDisplay::default();
    publish_analyzer(
        &magnitudes,
        &mut output,
        &display,
        48_000.0,
        DEFAULT_FFT_SIZE,
        1,
        1.0,
        0.5,
    );
    assert!(output.iter().any(|db| *db > -90.0));
    assert!(output.iter().all(|db| db.is_finite()));
}

#[test]
fn voice_ids_keep_overlapping_notes_independent() {
    let mut plugin = SpectralPlugin::default();
    plugin.start_voice(Some(10), 1, 60, 1.0);
    plugin.start_voice(Some(11), 1, 60, 1.0);
    plugin.release_voice(VoiceID::ID(10), Channel::Number(1), Key::Number(60));

    let first = plugin
        .voices
        .iter()
        .find(|voice| voice.voice_id == Some(10))
        .unwrap();
    let second = plugin
        .voices
        .iter()
        .find(|voice| voice.voice_id == Some(11))
        .unwrap();
    assert!(!first.held);
    assert!(second.held);
}

#[test]
fn expression_preparation_combines_note_and_channel_pitch() {
    let mut voices = [VoiceState::EMPTY; MAX_VOICES];
    voices[0] = VoiceState {
        occupied: true,
        held: true,
        channel: 3,
        note: 69,
        level: 1.0,
        tuning_semitones: 0.25,
        native_timbre: Some(0.8),
        ..VoiceState::EMPTY
    };
    let mut midi = MidiExpression::default();
    midi.pitch_bend(3, 1.0);
    midi.resolve(&mut voices, 2.0);
    let mut mask_voices = [MaskVoice::default(); MIDI_NOTES + 1];
    let mut levels = [0.0; MIDI_NOTES];
    let mut tunings = [0.0; MIDI_NOTES];
    let mut timbres = [0.5; MIDI_NOTES];
    let pinned = [0.0; MIDI_NOTES];
    prepare_mask_voices(
        &voices[..1],
        &mut mask_voices,
        0.0,
        0.0,
        &mut levels,
        &mut tunings,
        &mut timbres,
        &pinned,
        &mts_client::Tuning::default(),
    );
    assert!((mask_voices[0].tuning_semitones - 2.25).abs() < 1.0e-6);
    assert!((tunings[69] - 2.25).abs() < 1.0e-6);
    assert!((timbres[69] - 0.8).abs() < 1.0e-6);
}

#[test]
fn note_attack_and_release_use_independent_times() {
    let mut voices = [VoiceState {
        occupied: true,
        held: true,
        ..VoiceState::EMPTY
    }];
    advance_voices(&mut voices, 0.01, 100.0, 1000.0);
    let attacked = voices[0].level;
    voices[0].held = false;
    advance_voices(&mut voices, 0.01, 100.0, 1000.0);
    assert!(attacked > 0.09);
    assert!(voices[0].level > attacked * 0.98);
}

#[test]
fn mts_tunes_live_and_pinned_notes_adds_expression_and_filters_mapping() {
    let voices = [VoiceState {
        occupied: true,
        note: 69,
        channel: 2,
        level: 1.0,
        expression: VoiceExpression {
            tuning_semitones: 1.5,
            ..VoiceExpression::DEFAULT
        },
        ..VoiceState::EMPTY
    }];
    let mut targets = [MaskVoice::default(); MIDI_NOTES + 1];
    let mut levels = [0.0; 128];
    let mut tunings = [0.0; 128];
    let mut timbres = [0.0; 128];
    let mut pinned = [0.0; 128];
    pinned[69] = 1.0;
    let mut tuning = mts_client::Tuning {
        active: true,
        ..Default::default()
    };
    tuning.frequencies[69] = 880.0;
    prepare_mask_voices(
        &voices,
        &mut targets,
        0.0,
        0.0,
        &mut levels,
        &mut tunings,
        &mut timbres,
        &pinned,
        &tuning,
    );
    assert!((targets[0].tuning_semitones - 13.5).abs() < 0.0001);
    assert!((targets[70].tuning_semitones - 12.0).abs() < 0.0001);
    tuning.mapped[69] = false;
    prepare_mask_voices(
        &voices,
        &mut targets,
        0.0,
        0.0,
        &mut levels,
        &mut tunings,
        &mut timbres,
        &pinned,
        &tuning,
    );
    assert_eq!(targets[0].level, 0.0);
    assert_eq!(targets[70].level, 0.0);
    assert_eq!(levels[69], 0.0);
    prepare_mask_voices(
        &voices,
        &mut targets,
        0.0,
        0.0,
        &mut levels,
        &mut tunings,
        &mut timbres,
        &pinned,
        &Default::default(),
    );
    assert_eq!(targets[0].tuning_semitones, 1.5);
    assert_eq!(targets[70].tuning_semitones, 0.0);
}

#[test]
fn sustain_releases_only_pedal_held_voices_on_its_channel() {
    let mut plugin = SpectralPlugin::default();
    plugin.start_voice(Some(1), 1, 60, 1.0);
    plugin.start_voice(Some(2), 2, 60, 1.0);
    plugin.set_sustain(1, true);
    plugin.release_voice(VoiceID::ID(1), Channel::Number(1), Key::Number(60));
    plugin.release_voice(VoiceID::ID(2), Channel::Number(2), Key::Number(60));
    advance_voices(&mut plugin.voices, 0.1, 0.0, 0.0);
    assert!(plugin.voices[0].occupied && plugin.voices[0].sustained);
    assert!(!plugin.voices[1].occupied);
    plugin.start_voice(Some(3), 1, 64, 1.0);
    plugin.set_sustain(1, false);
    advance_voices(&mut plugin.voices, 0.1, 0.0, 0.0);
    assert!(!plugin.voices[0].occupied);
    assert!(
        plugin
            .voices
            .iter()
            .any(|voice| voice.held && voice.note == 64)
    );
}

#[test]
fn choke_overrides_sustain_and_reset_clears_the_pedal() {
    let mut plugin = SpectralPlugin::default();
    plugin.set_sustain(0, true);
    plugin.start_voice(Some(1), 0, 60, 1.0);
    plugin.release_voice(VoiceID::ID(1), Channel::Number(0), Key::Number(60));
    plugin.choke_voice(VoiceID::ID(1), Channel::Number(0), Key::Number(60));
    assert!(!plugin.voices[0].occupied);
    plugin.clear_notes();
    assert!(!plugin.channel_sustain[0]);
}

#[test]
fn displayed_note_level_mirrors_four_frame_hann_handover() {
    let mut history = [[0.0; MIDI_NOTES]; OVERLAP_TIMES];
    let mut current = [0.0; MIDI_NOTES];
    let mut displayed = [0.0; MIDI_NOTES];
    current[60] = 1.0;

    update_displayed_note_levels(&mut history, &current, &mut displayed);
    assert!((displayed[60] - 0.014_297_74).abs() < 1.0e-6);
    update_displayed_note_levels(&mut history, &current, &mut displayed);
    assert!((displayed[60] - 0.5).abs() < 1.0e-6);
    update_displayed_note_levels(&mut history, &current, &mut displayed);
    assert!((displayed[60] - 0.985_702_3).abs() < 1.0e-6);
    update_displayed_note_levels(&mut history, &current, &mut displayed);
    assert!((displayed[60] - 1.0).abs() < 1.0e-6);
}

#[test]
fn pinned_notes_round_trip_and_follow_the_note_envelope() {
    let pinned = PinnedNotesState::default();
    pinned.set(60, true);
    pinned.toggle_capture_incoming();
    let snapshot = PersistentField::map(&pinned, Clone::clone);
    let restored = PinnedNotesState::default();
    PersistentField::set(&restored, snapshot);
    assert!(restored.get(60));
    assert!(!restored.get(61));
    assert!(!restored.capture_incoming());

    let mut levels = [0.0; MIDI_NOTES];
    advance_pinned_notes(&mut levels, &restored, 0.01, 100.0, 1000.0);
    assert!(levels[60] > 0.09);
    assert_eq!(levels[61], 0.0);
}

#[test]
fn hold_mode_pins_incoming_notes() {
    let mut plugin = SpectralPlugin::default();
    plugin.params.pinned_notes.toggle_capture_incoming();
    plugin.start_voice(None, 0, 64, 1.0);
    plugin.release_voice(VoiceID::Wildcard, Channel::Number(0), Key::Number(64));
    assert!(plugin.params.pinned_notes.get(64));
    plugin.params.pinned_notes.toggle_capture_incoming();
    assert!(!plugin.params.pinned_notes.get(64));
}

#[test]
fn gui_can_remove_a_note_captured_by_hold() {
    let pinned = PinnedNotesState::default();
    pinned.toggle_capture_incoming();
    pinned.capture(64);
    assert!(pinned.get(64));

    pinned.set_from_ui(64, false);

    assert!(!pinned.get(64));
    assert!(pinned.capture_incoming());
}

#[test]
fn releasing_hold_keeps_manual_piano_notes() {
    let notes = PinnedNotesState::default();
    notes.set(60, true);
    notes.toggle_capture_incoming();
    notes.capture(64);
    assert!(notes.get(60));
    assert!(notes.get(64));
    notes.toggle_capture_incoming();
    assert!(notes.get(60));
    assert!(!notes.get(64));
}

#[test]
fn enabling_hold_captures_notes_that_are_already_down() {
    let mut plugin = SpectralPlugin::default();
    plugin.start_voice(None, 0, 67, 1.0);
    assert!(!plugin.params.pinned_notes.get(67));
    plugin.params.pinned_notes.toggle_capture_incoming();
    plugin.capture_held_notes_when_hold_starts();
    assert!(plugin.params.pinned_notes.get(67));
}

#[test]
fn curve_depth_scales_attenuation_without_lowering_the_ceiling() {
    let mut curve = [0.0; MANUAL_MASK_POINTS];
    curve[MANUAL_MASK_POINTS / 2] = -24.0;
    assert_eq!(
        transformed_curve_db(&curve, 0, 48_000.0, 50.0, 0.0, 0.0),
        0.0
    );
    assert_eq!(
        transformed_curve_db(&curve, MANUAL_MASK_POINTS / 2, 48_000.0, 50.0, 0.0, 0.0,),
        -12.0
    );
}

#[test]
fn tilt_preview_keeps_its_shape_above_zero_db() {
    let curve = [0.0; MANUAL_MASK_POINTS];
    let two_khz = (2_000.0 / 24_000.0 * (MANUAL_MASK_POINTS - 1) as f32).round() as usize;
    let four_khz = (4_000.0 / 24_000.0 * (MANUAL_MASK_POINTS - 1) as f32).round() as usize;
    let lower = transformed_curve_db(&curve, two_khz, 48_000.0, 100.0, 6.0, 0.0);
    let upper = transformed_curve_db(&curve, four_khz, 48_000.0, 100.0, 6.0, 0.0);

    assert!(lower > 0.0);
    assert!(upper > lower);
}

#[test]
fn processing_caps_stored_positive_curve_values_at_zero_db() {
    let mut source = [0.0; MANUAL_MASK_POINTS];
    source[42] = 18.0;
    source[43] = -12.0;
    let mut output = [0.0; MANUAL_MASK_POINTS];
    transform_curve(
        &source,
        &mut output,
        &[0.0; MANUAL_MASK_POINTS],
        100.0,
        0.0,
        0.0,
    );

    assert_eq!(output[42], 0.0);
    assert_eq!(output[43], -12.0);
    assert_eq!(source[42], 18.0);
}

#[test]
fn positive_curve_shift_moves_a_feature_one_octave_higher() {
    let mut curve = [0.0; MANUAL_MASK_POINTS];
    curve[MANUAL_MASK_POINTS / 4] = -18.0;
    assert_eq!(
        transformed_curve_db(&curve, MANUAL_MASK_POINTS / 2, 48_000.0, 100.0, 0.0, 12.0,),
        -18.0
    );
}
