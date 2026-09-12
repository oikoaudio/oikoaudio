use super::*;

#[test]
fn every_shape_is_periodic_and_bounded() {
    for shape in [-1.0, -0.5, 0.0, 0.5, 1.0] {
        let start = shape_primitive(0.37, shape);
        let end = shape_primitive(0.37 + TAU, shape);
        assert!((start - end).abs() < 1.0e-12);
        for index in 0..4096 {
            let phase = TAU * index as f64 / 4096.0;
            assert!(shape_primitive(phase, shape).abs() <= MAX_SHAPE_PRIMITIVE + 1.0e-12);
        }
    }
}

#[test]
fn sine_depth_maps_to_requested_speed_excursion() {
    let sample_rate = 48_000.0;
    let rate = 0.55;
    let depth = 24.0;
    let phase = 0.731;
    let step = TAU * rate / sample_rate;
    let d0 = oscillator_delay(phase, rate, depth, 0.0, sample_rate);
    let d1 = oscillator_delay(phase + step, rate, depth, 0.0, sample_rate);
    let measured = -(d1 - d0);
    let expected = speed_delta(depth) * phase.sin();
    assert!((measured - expected).abs() < 2.0e-6);
}

#[test]
fn smooth_random_repeats_after_reset() {
    let mut random = SmoothRandom::new(42);
    let first: Vec<_> = (0..2000).map(|_| random.next(100.0, 1000.0)).collect();
    random.reset();
    let second: Vec<_> = (0..2000).map(|_| random.next(100.0, 1000.0)).collect();
    assert_eq!(first, second);
    assert!(first.iter().all(|value| (-1.0..=1.0).contains(value)));
}

#[test]
fn stereo_is_linked_at_zero_strength() {
    let mut engine = ModulationEngine::new(48_000.0, 7);
    let params = ModulationParams {
        flutter_depth_cents: 12.0,
        drift_amount: 1.0,
        ..ModulationParams::default()
    };
    for _ in 0..10_000 {
        let offset = engine.next(params);
        assert_eq!(offset.left, offset.right);
    }
}

#[test]
fn stereo_phase_reaches_audible_channel_separation() {
    let mut engine = ModulationEngine::new(48_000.0, 7);
    let params = ModulationParams {
        wow_depth_cents: 20.0,
        stereo_amount: 1.0,
        ..ModulationParams::default()
    };
    let maximum_difference = (0..48_000)
        .map(|_| {
            let offset = engine.next(params);
            (offset.left - offset.right).abs()
        })
        .fold(0.0_f64, f64::max);
    assert!(maximum_difference > 100.0);
}

#[test]
fn drift_is_seeded_and_repeatable() {
    let params = ModulationParams {
        wow_depth_cents: 20.0,
        flutter_depth_cents: 8.0,
        drift_amount: 1.0,
        ..ModulationParams::default()
    };
    let mut first = ModulationEngine::new(48_000.0, 42);
    let mut second = ModulationEngine::new(48_000.0, 42);
    for _ in 0..48_000 {
        let a = first.next(params);
        let b = second.next(params);
        assert_eq!(a.left, b.left);
        assert_eq!(a.right, b.right);
    }
}

#[test]
fn drift_sources_are_independent() {
    let mut wow = SmoothRandom::new(19);
    let mut flutter = SmoothRandom::new(19 ^ 0xD1B5_4A32_D192_ED03);
    assert!((0..1000).any(|_| wow.next(1.0, 1000.0) != flutter.next(1.0, 1000.0)));
}

#[test]
fn combined_rate_bound_covers_each_endpoint() {
    let combined = maximum_rate_delta(&[60.0, 20.0]);
    assert!(combined >= speed_delta(60.0) * SQRT_2);
    assert!(combined >= speed_delta(20.0) * SQRT_2);
}

#[test]
fn phase_automation_crosses_zero_by_the_short_route_at_every_sample_rate() {
    for sample_rate in [44_100.0, 48_000.0, 96_000.0] {
        for (start, end, direction) in [(359.0, 1.0, 1.0), (1.0, 359.0, -1.0)] {
            let mut engine = ModulationEngine::new(sample_rate, 7);
            let mut params = ModulationParams {
                phase_offset_turns: start / 360.0,
                ..Default::default()
            };
            engine.next(params);
            let mut previous = engine.phase_offset.unwrap();
            params.phase_offset_turns = end / 360.0;
            let mut travel = 0.0;
            for _ in 0..(sample_rate * 0.3) as usize {
                engine.next(params);
                let current = engine.phase_offset.unwrap();
                let delta = circular_delta(current - previous);
                assert!(delta * direction >= -1e-12);
                assert!(delta.abs() < 0.00005);
                travel += delta;
                previous = current;
            }
            assert!((travel - direction * 2.0_f64.to_radians()).abs() < 1e-7);
        }
    }
}

#[test]
fn reanchoring_is_continuous_and_leaves_the_free_oscillator_running() {
    let mut engine = ModulationEngine::new(48_000.0, 42);
    let params = ModulationParams {
        drift_amount: 0.0,
        ..Default::default()
    };
    for _ in 0..1000 {
        engine.next(params);
    }
    let expected = oscillator_delay(
        engine.wow_phase,
        params.wow_rate_hz,
        params.wow_depth_cents,
        params.wow_shape,
        48_000.0,
    );
    let free_phase = engine.flutter_phase;
    engine.anchor_phases([Some(0.73), None], false);
    assert_eq!(engine.flutter_phase, free_phase);
    assert!((engine.next(params).left - expected).abs() < 1e-10);
    for _ in 0..20_000 {
        engine.next(params);
    }
    assert!(engine.phase_correction[0].abs() < 1e-8);
}

#[test]
fn phase_offset_and_stereo_spread_keep_their_independent_geometry() {
    let mut engine = ModulationEngine::new(48_000.0, 7);
    let params = ModulationParams {
        phase_offset_turns: 0.25,
        stereo_amount: 1.0,
        ..Default::default()
    };
    let output = engine.next(params);
    let amplitude = oscillator_delay(
        0.0,
        params.wow_rate_hz,
        params.wow_depth_cents,
        params.wow_shape,
        48_000.0,
    );
    assert!((output.left - amplitude).abs() < 1e-10);
    assert!((output.right + amplitude).abs() < 1e-10);
    engine.reset();
    let repeated = engine.next(params);
    assert_eq!(output.left, repeated.left);
    assert_eq!(output.right, repeated.right);
}
