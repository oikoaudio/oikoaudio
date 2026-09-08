use super::*;
use oiko_dsp::note_frequency;

fn flat_curve(db: f32) -> [f32; MANUAL_MASK_POINTS] {
    [db; MANUAL_MASK_POINTS]
}

#[test]
fn midi_note_mapping_uses_a440() {
    assert!((note_frequency(69) - 440.0).abs() < 1.0e-4);
    assert!((note_frequency(57) - 220.0).abs() < 1.0e-4);
    assert!((note_frequency_with_tuning(69, 1.0) - 466.163_76).abs() < 1.0e-3);
}

#[test]
fn per_voice_tuning_moves_the_spectral_peak() {
    let config = MaskConfig {
        sample_rate: 48_000.0,
        fft_size: 16_384,
        note_depth_db: 80.0,
        width_cents: 25.0,
        ..MaskConfig::default()
    };
    let voice = MaskVoice {
        note: 69,
        level: 1.0,
        tuning_semitones: 1.0,
        ..MaskVoice::default()
    };
    let mut mask = vec![0.0; config.fft_size / 2 + 1];
    build_mask_with_voices(&mut mask, &flat_curve(0.0), &[voice], config);
    let bent_bin = (note_frequency_with_tuning(69, 1.0) * config.fft_size as f32
        / config.sample_rate)
        .round() as usize;
    let unbent_bin = (440.0 * config.fft_size as f32 / config.sample_rate).round() as usize;
    assert!(mask[bent_bin] > 0.99);
    assert!(mask[unbent_bin] < 0.1);
}

#[test]
fn per_voice_timbre_changes_partial_rolloff() {
    let config = MaskConfig {
        sample_rate: 48_000.0,
        fft_size: 16_384,
        note_depth_db: 80.0,
        width_cents: 35.0,
        partials: 2,
        harmonic_rolloff_db_per_octave: 12.0,
        ..MaskConfig::default()
    };
    let mut dark = vec![0.0; config.fft_size / 2 + 1];
    let mut bright = vec![0.0; config.fft_size / 2 + 1];
    build_mask_with_voices(
        &mut dark,
        &flat_curve(0.0),
        &[MaskVoice {
            note: 69,
            level: 1.0,
            timbre: 0.0,
            ..MaskVoice::default()
        }],
        config,
    );
    build_mask_with_voices(
        &mut bright,
        &flat_curve(0.0),
        &[MaskVoice {
            note: 69,
            level: 1.0,
            timbre: 1.0,
            ..MaskVoice::default()
        }],
        config,
    );
    let octave_bin = (880.0 * config.fft_size as f32 / config.sample_rate).round() as usize;
    assert!(bright[octave_bin] > dark[octave_bin] * 8.0);
}

#[test]
fn precomputed_mask_matches_reference_path() {
    let config = MaskConfig {
        sample_rate: 48_000.0,
        fft_size: 16_384,
        note_depth_db: 72.0,
        width_cents: 137.0,
        partials: 24,
        harmonic_rolloff_db_per_octave: 7.5,
        ..MaskConfig::default()
    };
    let voices = [
        MaskVoice {
            note: 37,
            level: 0.83,
            tuning_semitones: 0.17,
            pressure: 0.72,
            timbre: 0.61,
            volume_gain: 0.91,
            ..MaskVoice::default()
        },
        MaskVoice {
            note: 64,
            level: 0.57,
            tuning_semitones: -0.23,
            ..MaskVoice::default()
        },
    ];
    let mut reference = vec![0.0; config.fft_size / 2 + 1];
    let mut precomputed = reference.clone();
    build_mask_with_voices(&mut reference, &flat_curve(-3.0), &voices, config);
    let mut workspace = MaskWorkspace::default();
    workspace.prepare(config.sample_rate, config.fft_size);
    build_mask_with_voices_precomputed(
        &mut precomputed,
        &flat_curve(-3.0),
        &voices,
        config,
        &workspace,
    );

    let maximum_error = reference
        .iter()
        .zip(precomputed)
        .map(|(reference, precomputed)| (reference - precomputed).abs())
        .fold(0.0_f32, f32::max);
    // Reordering log2(frequency / centre) as log2(frequency) -
    // log2(centre), plus interpolating the Gaussian table, stays well
    // below one thousandth of linear gain.
    assert!(maximum_error < 3.0e-4, "maximum error: {maximum_error}");
}

#[test]
fn flat_manual_curve_produces_constant_gain() {
    let mut mask = vec![0.0; 4096 / 2 + 1];
    build_mask(
        &mut mask,
        &flat_curve(-6.0),
        &[0.0; MIDI_NOTES],
        MaskConfig {
            fft_size: 4096,
            note_depth_db: 0.0,
            ..MaskConfig::default()
        },
    );
    let expected = db_to_gain(-6.0);
    assert!(mask.iter().all(|gain| (*gain - expected).abs() < 1.0e-5));
}

#[test]
fn midi_mask_peaks_at_held_note() {
    let config = MaskConfig {
        sample_rate: 48_000.0,
        fft_size: 16_384,
        note_depth_db: 80.0,
        width_cents: 50.0,
        ..MaskConfig::default()
    };
    let mut notes = [0.0; MIDI_NOTES];
    notes[69] = 1.0;
    let mut mask = vec![0.0; config.fft_size / 2 + 1];
    build_mask(&mut mask, &flat_curve(0.0), &notes, config);

    let a440_bin = (440.0 * config.fft_size as f32 / config.sample_rate).round() as usize;
    let off_bin = (1000.0 * config.fft_size as f32 / config.sample_rate).round() as usize;
    assert!(mask[a440_bin] > 0.99);
    assert!(mask[off_bin] < 0.001);
}

#[test]
fn harmonic_mode_opens_octave_partial() {
    let config = MaskConfig {
        sample_rate: 48_000.0,
        fft_size: 16_384,
        note_depth_db: 80.0,
        width_cents: 40.0,
        partials: 2,
        harmonic_rolloff_db_per_octave: 0.0,
        ..MaskConfig::default()
    };
    let mut notes = [0.0; MIDI_NOTES];
    notes[69] = 1.0;
    let mut mask = vec![0.0; config.fft_size / 2 + 1];
    build_mask(&mut mask, &flat_curve(0.0), &notes, config);
    let octave_bin = (880.0 * config.fft_size as f32 / config.sample_rate).round() as usize;
    assert!(mask[octave_bin] > 0.99);
}

#[test]
fn envelope_fades_on_and_off_monotonically() {
    let mut levels = [0.0; MIDI_NOTES];
    let mut held = [false; MIDI_NOTES];
    held[60] = true;
    advance_note_envelopes(&mut levels, &held, 0.01, 50.0);
    let attacked = levels[60];
    assert!(attacked > 0.0 && attacked < 1.0);
    held[60] = false;
    advance_note_envelopes(&mut levels, &held, 0.01, 50.0);
    assert!(levels[60] < attacked);
}

#[test]
fn smaller_fft_sizes_address_exact_master_bin_subsets() {
    let mut mask = [0.0; MANUAL_MASK_POINTS];
    // Bin 100 in an 8192 FFT maps to master bin 200.
    mask[200] = -12.0;
    let gain = manual_gain_at_bin(&mask, 100, 8192);
    assert!((gain - db_to_gain(-12.0)).abs() < 1.0e-6);
}

#[test]
fn manual_curve_reaches_exact_silence_at_minus_144_db() {
    let mut mask = [0.0; MANUAL_MASK_POINTS];
    mask[200] = MANUAL_CURVE_MUTE_DB + 1.0;
    assert!(manual_gain_at_bin(&mask, 100, 8192) > 0.0);

    mask[200] = MANUAL_CURVE_MUTE_DB;
    assert_eq!(manual_gain_at_bin(&mask, 100, 8192), 0.0);
}

#[test]
fn note_depth_emphasizes_open_regions_but_remains_bounded() {
    let mut notes = [0.0; MIDI_NOTES];
    notes[69] = 1.0;
    let mut mask = vec![0.0; 4096 / 2 + 1];
    build_mask(
        &mut mask,
        &flat_curve(-12.0),
        &notes,
        MaskConfig {
            fft_size: 4096,
            note_depth_db: 24.0,
            ..MaskConfig::default()
        },
    );
    let a440_bin = (440.0_f32 * 4096.0 / 48_000.0).round() as usize;
    let off_bin = (1000.0_f32 * 4096.0 / 48_000.0).round() as usize;
    let manual = db_to_gain(-12.0);
    let maximum = manual * note_emphasis_gain(100.0);
    assert!(mask[a440_bin] > manual);
    assert!(mask[a440_bin] <= maximum + 1.0e-6);
    assert!((mask[off_bin] - db_to_gain(-36.0)).abs() < 0.001);
    assert!(mask.iter().all(|gain| *gain <= maximum + 1.0e-6));
}

#[test]
fn notes_are_additive_at_zero_depth() {
    let mut notes = [0.0; MIDI_NOTES];
    notes[69] = 1.0;
    let config = MaskConfig {
        fft_size: 4096,
        note_depth_db: 0.0,
        width_cents: 100.0,
        ..MaskConfig::default()
    };
    let mut mask = vec![0.0; config.fft_size / 2 + 1];
    build_mask(&mut mask, &flat_curve(-12.0), &notes, config);
    let a440_bin = (440.0 * config.fft_size as f32 / config.sample_rate).round() as usize;
    let off_bin = (1000.0 * config.fft_size as f32 / config.sample_rate).round() as usize;
    let manual = db_to_gain(-12.0);
    assert!((mask[a440_bin] - manual * db_to_gain(6.0)).abs() < 1.0e-5);
    assert!((mask[off_bin] - manual).abs() < 0.001);
}

#[test]
fn fractional_bin_note_reaches_full_peak_at_coarse_resolution() {
    let config = MaskConfig {
        sample_rate: 48_000.0,
        fft_size: 1024,
        note_depth_db: 90.0,
        width_cents: 10.0,
        ..MaskConfig::default()
    };
    let mut notes = [0.0; MIDI_NOTES];
    notes[69] = 1.0;
    let mut mask = vec![0.0; config.fft_size / 2 + 1];
    build_mask(&mut mask, &flat_curve(0.0), &notes, config);
    let peak = mask.iter().copied().fold(0.0_f32, f32::max);
    let expected = note_emphasis_gain(config.width_cents);
    assert!((peak - expected).abs() < 1.0e-5);
}

#[test]
fn zero_motion_depth_is_bit_identical_to_the_unmodulated_mask() {
    let mut baseline = vec![0.0; 4096 / 2 + 1];
    let mut motion = vec![0.0; baseline.len()];
    let config = MaskConfig {
        fft_size: 4096,
        ..MaskConfig::default()
    };
    build_mask(&mut baseline, &flat_curve(-6.0), &[0.0; MIDI_NOTES], config);
    build_mask(
        &mut motion,
        &flat_curve(-6.0),
        &[0.0; MIDI_NOTES],
        MaskConfig {
            motion: MotionConfig {
                shape: MotionShape::Drift,
                depth_db: 0.0,
                phase: 0.73,
                size_octaves: 0.25,
            },
            ..config
        },
    );
    assert_eq!(baseline, motion);
}

#[test]
fn motion_compensation_is_bounded_and_preserves_contrast() {
    let config = MaskConfig {
        fft_size: 4096,
        motion: MotionConfig {
            shape: MotionShape::Ripple,
            depth_db: 24.0,
            phase: 0.31,
            size_octaves: 1.0,
        },
        ..MaskConfig::default()
    };
    let mut mask = vec![0.0; config.fft_size / 2 + 1];
    build_mask(&mut mask, &flat_curve(-9.0), &[0.0; MIDI_NOTES], config);
    let ceiling = db_to_gain(-9.0);
    let maximum = ceiling * db_to_gain(MAX_MOTION_COMPENSATION_DB);
    assert!(mask.iter().all(|gain| *gain <= maximum + 1.0e-6));
    assert!(mask.iter().any(|gain| *gain < db_to_gain(-25.0)));
}

#[test]
fn motion_compensation_normalizes_each_shape_at_moderate_depth() {
    assert_eq!(motion_compensation_gain(MotionConfig::default()), 1.0);
    for shape in [
        MotionShape::Ripple,
        MotionShape::Harmonic,
        MotionShape::Drift,
        MotionShape::Scan,
        MotionShape::Notch,
        MotionShape::Saw,
        MotionShape::Splash,
    ] {
        let config = MotionConfig {
            shape,
            depth_db: 6.0,
            phase: 0.37,
            size_octaves: 0.8,
        };
        let compensation = motion_compensation_gain(config);
        assert!(compensation >= 1.0);
        assert!(compensation <= db_to_gain(MAX_MOTION_COMPENSATION_DB));
        let compensated_power = (0..MOTION_COMPENSATION_SAMPLES)
            .map(|index| {
                let octave =
                    (index as f32 + 0.5) / MOTION_COMPENSATION_SAMPLES as f32 * DISPLAY_OCTAVES;
                let frequency = MIN_DISPLAY_FREQUENCY_HZ * 2.0_f32.powf(octave);
                let gain = db_to_gain(-motion_attenuation_db(frequency, config)) * compensation;
                gain * gain
            })
            .sum::<f32>()
            / MOTION_COMPENSATION_SAMPLES as f32;
        assert!((compensated_power.sqrt() - 1.0).abs() < 1.0e-5);
    }
}

#[test]
fn every_motion_shape_is_continuous_at_phase_wrap() {
    for shape in [
        MotionShape::Ripple,
        MotionShape::Harmonic,
        MotionShape::Drift,
        MotionShape::Scan,
        MotionShape::Notch,
        MotionShape::Saw,
        MotionShape::Splash,
    ] {
        for frequency in [31.0, 440.0, 7_321.0, 19_000.0] {
            let config = |phase| MotionConfig {
                shape,
                depth_db: 18.0,
                phase,
                size_octaves: 0.73,
            };
            let before = motion_openness(frequency, config(0.999_999));
            let after = motion_openness(frequency, config(0.000_001));
            assert!((before - after).abs() < 1.0e-3, "{shape:?} at {frequency}");
        }
    }
}

#[test]
fn splash_ring_moves_upward_from_its_note() {
    let at_impact = [SplashEvent {
        center_hz: 440.0,
        radius_octaves: 0.0,
        strength: 1.0,
    }];
    assert!(splash_openness(440.0, 1.0, &at_impact) > 0.99);
    assert!(splash_openness(880.0, 1.0, &at_impact) < 0.001);

    let one_octave_out = [SplashEvent {
        radius_octaves: 1.0,
        ..at_impact[0]
    }];
    assert!(splash_openness(880.0, 1.0, &one_octave_out) > 0.8);
    assert_eq!(splash_openness(220.0, 1.0, &one_octave_out), 0.0);
    assert!(splash_openness(440.0, 1.0, &one_octave_out) < 0.001);
}

#[test]
fn splash_is_neutral_without_an_active_event() {
    assert_eq!(splash_attenuation_db(440.0, 60.0, 1.0, &[]), 0.0);
    assert_eq!(
        motion_attenuation_db(
            440.0,
            MotionConfig {
                shape: MotionShape::Splash,
                depth_db: 60.0,
                phase: 0.5,
                size_octaves: 1.0,
            }
        ),
        0.0
    );
}

#[test]
fn zero_note_depth_adds_notes_without_dimming_the_background() {
    assert!((note_gate_gain(0.0, 100.0, 0.0) - 1.0).abs() < 1.0e-6);
    assert!((note_gate_gain(0.0, 100.0, 1.0) - db_to_gain(6.0)).abs() < 1.0e-6);
    assert!((note_gate_gain(24.0, 100.0, 0.0) - db_to_gain(-24.0)).abs() < 1.0e-6);
    assert!((note_gate_gain(90.0, 100.0, 1.0) - db_to_gain(6.0)).abs() < 1.0e-6);
}

#[test]
fn notch_is_the_inverse_of_scan() {
    for frequency in [31.0, 440.0, 7_321.0, 19_000.0] {
        let config = |shape| MotionConfig {
            shape,
            depth_db: 18.0,
            phase: 0.37,
            size_octaves: 0.8,
        };
        let scan = motion_openness(frequency, config(MotionShape::Scan));
        let notch = motion_openness(frequency, config(MotionShape::Notch));
        assert!((scan + notch - 1.0).abs() < 1.0e-6);
    }
}

#[test]
fn note_emphasis_tapers_to_unity_as_width_reaches_an_octave() {
    let narrow = note_emphasis_gain(100.0);
    let three_hundred = note_emphasis_gain(300.0);
    let four_hundred = note_emphasis_gain(400.0);
    let medium = note_emphasis_gain(650.0);
    let octave = note_emphasis_gain(1200.0);
    assert!(narrow > three_hundred);
    assert!(three_hundred > four_hundred);
    assert!(four_hundred > medium);
    assert!(medium > octave);
    assert!(three_hundred > db_to_gain(3.0));
    assert!(three_hundred < db_to_gain(4.0));
    assert!(medium < db_to_gain(1.5));
    assert!((octave - 1.0).abs() < 1.0e-6);
}
