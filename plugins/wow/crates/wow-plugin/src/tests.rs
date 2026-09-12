use super::*;

#[test]
fn legacy_state_restores_free_rates_and_current_state_keeps_sync() {
    let mut state = PluginState {
        version: "0.1.1".into(),
        params: [
            ("rate".into(), ParamValue::F32(0.73)),
            ("flutter_rate".into(), ParamValue::F32(17.2)),
        ]
        .into(),
        fields: Default::default(),
    };
    WowPlugin::filter_state(&mut state);
    assert!(matches!(state.params["phase_offset"], ParamValue::F32(0.0)));
    assert!(matches!(state.params["rate_sync"], ParamValue::Bool(false)));
    assert!(matches!(
        state.params["flutter_rate_sync"],
        ParamValue::Bool(false)
    ));
    assert!(matches!(state.params["rate"], ParamValue::F32(v) if v == 0.73));
    assert!(matches!(state.params["flutter_rate"], ParamValue::F32(v) if v == 17.2));
    state
        .params
        .insert("rate_sync".into(), ParamValue::Bool(true));
    state
        .params
        .insert("phase_offset".into(), ParamValue::F32(0.75));
    WowPlugin::filter_state(&mut state);
    assert!(matches!(
        state.params["phase_offset"],
        ParamValue::F32(0.75)
    ));
    assert!(matches!(state.params["rate_sync"], ParamValue::Bool(true)));
}

#[test]
fn interface_scale_round_trips_and_stays_within_supported_bounds() {
    let scale = UiScaleState::default();
    assert!((scale.get() - 1.0).abs() < 1.0e-6);
    scale.set(1.75);
    let snapshot = PersistentField::map(&scale, |value| *value);
    let restored = UiScaleState::default();
    PersistentField::set(&restored, snapshot);
    assert!((restored.get() - 1.75).abs() < 1.0e-6);

    restored.set(3.0);
    assert!((restored.get() - 2.0).abs() < 1.0e-6);
}

#[test]
fn exported_parameter_order_keeps_amount_on_the_main_page() {
    let ids: Vec<_> = WowParams::default()
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(
        ids,
        [
            "rate",
            "flutter_rate",
            "amount",
            "wow_flutter",
            "flux",
            "stereo",
            "random_seed",
            "quality",
            "depth_behavior",
            "rate_sync",
            "rate_division",
            "flutter_rate_sync",
            "flutter_rate_division",
            "phase_offset",
        ]
    );
}

#[test]
fn clap_identity_is_stable() {
    assert_eq!(WowPlugin::CLAP_ID, "com.oikoaudio.wow");
}

#[test]
fn cents_depth_maps_to_speed_excursion() {
    let sample_rate = 48_000.0;
    let rate = 0.5;
    let depth = 12.0;
    let amplitude = maximum_delay_excursion_samples(depth, rate, sample_rate)
        / wow_dsp::modulation::MAX_SHAPE_PRIMITIVE;
    let angular_rate = std::f64::consts::TAU * rate / sample_rate;
    let measured_speed_delta = amplitude * angular_rate;
    let expected = 2.0_f64.powf(depth / 1200.0) - 1.0;
    assert!((measured_speed_delta - expected).abs() < 1.0e-14);
}

#[test]
fn control_midpoints_preserve_the_host_parameter_mapping() {
    let params = WowParams::default();
    assert!((params.rate.range().unnormalize(0.5) - 0.6).abs() < 1.0e-6);
    assert!((params.flutter_rate.range().unnormalize(0.5) - 12.0).abs() < 1.0e-5);
    assert!((params.amount.range().unnormalize(0.5) - 0.5).abs() < 1.0e-6);
}

#[test]
fn balance_is_constant_power_with_calibrated_endpoints() {
    let wow = modulation_depths(
        1.0,
        0.0,
        MAX_RATE_HZ,
        MAX_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Pitch,
    );
    assert!((wow.0 - 60.0).abs() < 1.0e-12);
    assert_eq!(wow.1, 0.0);
    let centre = modulation_depths(
        1.0,
        0.5,
        MAX_RATE_HZ,
        MAX_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Pitch,
    );
    assert!((centre.0 - 60.0 / 2.0_f64.sqrt()).abs() < 1.0e-12);
    assert!((centre.1 - 20.0 / 2.0_f64.sqrt()).abs() < 1.0e-12);
    let flutter = modulation_depths(
        1.0,
        1.0,
        MAX_RATE_HZ,
        MAX_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Pitch,
    );
    assert!(flutter.0.abs() < 1.0e-12);
    assert_eq!(flutter.1, 20.0);
}

#[test]
fn time_behavior_keeps_delay_excursion_constant() {
    let slow = modulation_depths(
        1.0,
        0.0,
        MIN_RATE_HZ,
        MIN_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Time,
    )
    .0;
    let fast = modulation_depths(
        1.0,
        0.0,
        MAX_RATE_HZ,
        MAX_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Time,
    )
    .0;
    let slow_excursion = speed_delta(slow) / MIN_RATE_HZ;
    let fast_excursion = speed_delta(fast) / MAX_RATE_HZ;
    assert!((slow_excursion - fast_excursion).abs() < 1.0e-14);
    assert!((slow - 6.09).abs() < 0.01);
    assert!((fast - 228.449_123_085_978_73).abs() < 1.0e-12);

    let slow_musical = modulation_depths(
        1.0,
        0.0,
        0.4,
        MAX_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Time,
    )
    .0;
    assert!((slow_musical - 24.250_098_474_428_626).abs() < 1.0e-12);

    let current_maximum = modulation_depths(
        0.25,
        0.0,
        MAX_RATE_HZ,
        MAX_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Time,
    )
    .0;
    assert!((current_maximum - 60.0).abs() < 1.0e-12);
}

#[test]
fn balance_has_the_same_audible_ratio_in_both_depth_behaviors() {
    let wow_rate = 0.4;
    let flutter_rate = 12.0;
    let balance = 0.5;
    let rate_scaled = modulation_depths(
        1.0,
        balance,
        wow_rate,
        flutter_rate,
        PluginDepthBehavior::Time,
    );
    let constant = modulation_depths(
        1.0,
        balance,
        wow_rate,
        flutter_rate,
        PluginDepthBehavior::Pitch,
    );

    let rate_scaled_ratio = speed_delta(rate_scaled.0) / speed_delta(rate_scaled.1);
    let constant_ratio = speed_delta(constant.0) / speed_delta(constant.1);
    assert!((rate_scaled_ratio - constant_ratio).abs() < 1.0e-12);

    let theta = balance * std::f64::consts::FRAC_PI_2;
    let original_delay_budget =
        (speed_delta(MAX_DEPTH_CENTS) * TIME_DEPTH_SCALE * theta.cos() / MAX_RATE_HZ).hypot(
            speed_delta(MAX_FLUTTER_DEPTH_CENTS) * TIME_DEPTH_SCALE * theta.sin()
                / MAX_FLUTTER_RATE_HZ,
        );
    let normalized_delay_budget =
        (speed_delta(rate_scaled.0) / wow_rate).hypot(speed_delta(rate_scaled.1) / flutter_rate);
    assert!((normalized_delay_budget - original_delay_budget).abs() < 1.0e-14);
}

#[test]
fn pitch_behavior_tapers_depth_below_point_two_hz() {
    let at_floor = modulation_depths(
        1.0,
        0.0,
        MIN_CONSTANT_PITCH_RATE_HZ,
        MIN_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Pitch,
    )
    .0;
    let below_floor = modulation_depths(
        1.0,
        0.0,
        MIN_RATE_HZ,
        MIN_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Pitch,
    )
    .0;
    assert!((at_floor - 60.0).abs() < 1.0e-12);
    assert!(
        (speed_delta(below_floor) / MIN_RATE_HZ
            - speed_delta(at_floor) / MIN_CONSTANT_PITCH_RATE_HZ)
            .abs()
            < 1.0e-14
    );
    assert!((below_floor - 30.26).abs() < 0.01);
}

#[test]
fn oscillator_rates_have_distinct_wow_and_flutter_ranges() {
    let params = WowParams::default();
    assert_eq!(params.rate.range().unnormalize(0.0), MIN_RATE_HZ as f32);
    assert_eq!(params.rate.range().unnormalize(1.0), MAX_RATE_HZ as f32);
    assert_eq!(
        params.flutter_rate.range().unnormalize(0.0),
        MIN_FLUTTER_RATE_HZ as f32
    );
    assert_eq!(
        params.flutter_rate.range().unnormalize(1.0),
        MAX_FLUTTER_RATE_HZ as f32
    );
}

#[test]
fn stereo_defaults_to_linked() {
    let params = WowParams::default();
    assert_eq!(params.stereo.value(), 0.0);
    assert_eq!(params.depth_behavior.value(), PluginDepthBehavior::Time);
}

#[test]
fn depth_behaviors_have_expected_latency() {
    let sample_rate = 48_000.0;
    let time = base_delay_samples(PluginDepthBehavior::Time, sample_rate);
    let pitch = base_delay_samples(PluginDepthBehavior::Pitch, sample_rate);
    assert_eq!(time, 385);
    assert_eq!(pitch, 1642);
}

#[test]
fn delay_slew_never_exceeds_the_filter_bank_rate() {
    let maximum_step = maximum_supported_rate_delta();
    let mut delay = Some(1_642.0);
    let mut reached_target = false;

    for _ in 0..10_000 {
        let (next, playback_rate) = bounded_delay_step(delay, 385.0, maximum_step);
        assert!(playback_rate <= 1.0 + maximum_step + f64::EPSILON);
        assert!(playback_rate >= 1.0 - maximum_step - f64::EPSILON);
        delay = Some(next);
        if next == 385.0 {
            reached_target = true;
            break;
        }
    }

    assert!(reached_target);
}

#[test]
fn delay_slew_preserves_supported_motion_exactly() {
    let maximum_step = maximum_supported_rate_delta();
    let target = 512.0 - maximum_step * 0.5;
    let (delay, playback_rate) = bounded_delay_step(Some(512.0), target, maximum_step);
    assert_eq!(delay, target);
    assert_eq!(playback_rate, 1.0 - (target - 512.0));
}

#[test]
fn maximum_time_wow_audibly_changes_a_sine() {
    let sample_rate = 48_000.0;
    let maximum_excursion = maximum_delay_excursion(PluginDepthBehavior::Pitch, sample_rate);
    let maximum_base = base_delay_samples(PluginDepthBehavior::Pitch, sample_rate);
    let max_delay = (maximum_base as f64 + maximum_excursion).ceil() as usize;
    let bank = Arc::new(SincBankF32::for_max_rate(
        1.0 + maximum_supported_rate_delta(),
    ));
    let mut modulated =
        QualityVariableDelayF32::new(max_delay, KERNEL_MARGIN, bank.clone(), QualityMode::Hq, 0);
    let mut fixed =
        QualityVariableDelayF32::new(max_delay, KERNEL_MARGIN, bank, QualityMode::Hq, 0);
    let mut modulation = ModulationEngine::new(sample_rate, 1);
    let base = base_delay_samples(PluginDepthBehavior::Time, sample_rate) as f64;
    let (wow_depth_cents, flutter_depth_cents) = modulation_depths(
        1.0,
        0.0,
        MAX_RATE_HZ,
        MIN_FLUTTER_RATE_HZ,
        PluginDepthBehavior::Time,
    );
    let params = ModulationParams {
        wow_rate_hz: MAX_RATE_HZ,
        wow_depth_cents,
        flutter_rate_hz: MIN_FLUTTER_RATE_HZ,
        flutter_depth_cents,
        ..ModulationParams::default()
    };
    let mut squared_difference = 0.0_f64;
    let mut count = 0;
    let mut previous_offset = None;
    for index in 0..48_000 {
        let input =
            (0.5 * (std::f64::consts::TAU * 3_150.0 * index as f64 / sample_rate).sin()) as f32;
        let offset = modulation.next(params).left;
        let playback_rate = previous_offset
            .map(|previous| 1.0 - (offset - previous))
            .unwrap_or(1.0);
        previous_offset = Some(offset);
        let moving = modulated.process_sample_rate_aware(
            input,
            base + offset,
            QualityMode::Hq,
            playback_rate,
        );
        let stationary = fixed.process_sample(input, base, QualityMode::Hq);
        if index >= 4_000 {
            squared_difference += (moving - stationary) as f64 * (moving - stationary) as f64;
            count += 1;
        }
    }
    let difference_rms = (squared_difference / count as f64).sqrt();
    assert!(difference_rms > 0.1, "difference RMS was {difference_rms}");
}
