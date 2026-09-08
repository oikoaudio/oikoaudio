use super::*;

#[test]
fn interpolation_is_exact_at_integer_positions() {
    let data = [0.0, 1.0, -2.0, 3.5, 0.25, -1.0, 2.0, 0.0];
    let source = SliceSource(&data);
    assert!((CubicFarrow.sample(&source, 3.0) - 3.5).abs() < 1e-14);
    assert!((Lagrange::<8>.sample(&source, 3.0) - 3.5).abs() < 1e-12);
    assert!((Farrow8.sample(&source, 3.0) - 3.5).abs() < 1e-12);
}

#[test]
fn fixed_farrow_matches_generic_lagrange() {
    let data: Vec<_> = (0..128)
        .map(|n| ((n as f64 * 0.731).sin() + (n as f64 * 0.173).cos()) * 0.5)
        .collect();
    let source = SliceSource(&data);
    for &position in &[16.01, 23.125, 47.5, 79.9, 110.999] {
        let expected = Lagrange::<8>.sample(&source, position);
        let actual = Farrow8.sample(&source, position);
        assert!(
            (actual - expected).abs() < 2.0e-13,
            "position={position} actual={actual} expected={expected} error={}",
            (actual - expected).abs()
        );
    }
}

#[test]
fn sinc_table_has_unity_dc_gain() {
    let kernel = WindowedSinc::new(64, 1024, 0.94, 10.0);
    let ones = vec![1.0; 256];
    let source = SliceSource(&ones);
    for &fraction in &[0.01, 0.25, 0.5, 0.9, 0.99] {
        assert!((kernel.sample(&source, 128.0 + fraction) - 1.0).abs() < 1e-12);
    }
}

#[test]
fn ring_wrap_preserves_constant_signal() {
    let mut delay = VariableDelay::new(128, 40, WindowedSinc::new(32, 128, 0.94, 9.0));
    let mut last = 0.0;
    for _ in 0..2000 {
        last = delay.process_sample(1.0, 64.25);
    }
    assert!((last - 1.0).abs() < 1e-10);
}

#[test]
fn rate_aware_sinc_reserves_the_safe_band() {
    let kernel = WindowedSinc::for_max_rate(64, 256, 1.25, 0.95, 10.0);
    assert!((kernel.cutoff() - 0.76).abs() < 1e-12);
}

#[test]
fn oversampled_reader_preserves_dc_after_settling() {
    let mut reader = OversampledVariableDelay::<_, 2>::new(256, 8, 129, CubicFarrow);
    let mut output = 0.0;
    for _ in 0..4000 {
        output = reader.process_sample(1.0, 128.25);
    }
    assert!((output - 1.0).abs() < 1e-10);
    assert_eq!(reader.latency_samples(), 64.0);
}

#[test]
fn cascaded_halfband_reader_preserves_dc_after_settling() {
    let mut reader = CascadedOversampledVariableDelay4x::new(256, 8, CubicFarrow);
    let mut output = 0.0;
    for _ in 0..4000 {
        output = reader.process_sample(1.0, 128.25);
    }
    assert!((output - 1.0).abs() < 1e-10);
    assert_eq!(reader.latency_samples(), 64.0);
}

#[test]
fn f32_sinc_reader_preserves_dc_after_settling() {
    let mut reader = VariableDelayF32::new(256, 52, WindowedSincF32::new(96, 4096, 0.98, 14.0));
    let mut output = 0.0;
    for _ in 0..4000 {
        output = reader.process_sample(1.0, 128.25);
    }
    assert!((output - 1.0).abs() < 2.0e-6);
}

#[test]
fn quality_switch_uses_shared_history() {
    let bank = Arc::new(SincBankF32::for_max_rate(1.02));
    let mut reader = QualityVariableDelayF32::new(256, 68, bank, QualityMode::Normal, 32);
    let mut output = 0.0;
    for index in 0..4000 {
        let quality = if index < 2000 {
            QualityMode::Normal
        } else {
            QualityMode::Ultra
        };
        output = reader.process_sample(1.0, 128.25, quality);
    }
    assert!((output - 1.0).abs() < 2.0e-6);
}
