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

struct Samples<'a>(&'a [f32]);

impl SampleSourceF32 for Samples<'_> {
    fn at_f32(&self, index: i64) -> f32 {
        self.0[index.clamp(0, self.0.len() as i64 - 1) as usize]
    }
}

fn tone(frames: usize, cycles_per_sample: f64) -> Vec<f32> {
    (0..frames)
        .map(|n| (std::f64::consts::TAU * cycles_per_sample * n as f64).sin() as f32)
        .collect()
}

fn noise(frames: usize) -> Vec<f32> {
    let mut state = 1_u32;
    (0..frames)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1 << 23) as f32 - 1.0
        })
        .collect()
}

fn read_level_db(reader: &RateSincF32, sound: &[f32], speed: f64) -> f64 {
    const READS: usize = 400;
    let power = (0..READS)
        .map(|read| {
            let position = sound.len() as f64 / 2.0 + read as f64 * speed + 0.37;
            f64::from(reader.sample(&Samples(sound), position, speed)).powi(2)
        })
        .sum::<f64>()
        / READS as f64;
    10.0 * (power * 2.0).max(1.0e-30).log10()
}

fn live_reader() -> RateSincF32 {
    RateSincF32::new(48, 256, 16.0, 0.87, 9.0)
}

fn best_reader() -> RateSincF32 {
    RateSincF32::new(128, 4096, 16.0, 0.919, 16.0)
}

#[test]
fn bank_reads_a_place_as_its_delay_line_does() {
    let bank = Arc::new(SincBankF32::for_max_rate(1.1));
    let input = noise(1024);
    let mut delay = QualityVariableDelayF32::new(256, 68, bank.clone(), QualityMode::Hq, 0);
    let delay_samples = 128.25;
    for (index, &sample) in input.iter().enumerate() {
        let delayed = delay.process_sample_rate_aware(sample, delay_samples, QualityMode::Hq, 1.05);
        if index >= 512 {
            let position = index as f64 - delay_samples;
            let read = bank.sample(&Samples(&input), position, QualityMode::Hq, 1.05);
            assert_eq!(read.to_bits(), delayed.to_bits());
        }
    }
}

struct Span(std::cell::Cell<(i64, i64)>);

impl Span {
    fn new() -> Self {
        Self(std::cell::Cell::new((i64::MAX, i64::MIN)))
    }
}

impl SampleSourceF32 for Span {
    fn at_f32(&self, index: i64) -> f32 {
        let (low, high) = self.0.get();
        self.0.set((low.min(index), high.max(index)));
        0.0
    }
}

#[test]
fn table_taps_cover_every_sample_a_read_touches() {
    let kernel = WindowedSincF32::new(32, 64, 0.9, 9.0);
    let span = Span::new();
    kernel.sample_f32(&span, 100.25);
    let (low, high) = span.0.get();
    assert_eq!((high - low + 1) as usize, kernel.taps());
}

#[test]
fn rate_read_rejects_tones_past_the_output_nyquist() {
    for (reader, floor) in [(live_reader(), -90.0), (best_reader(), -140.0)] {
        for speed in [2.0, 3.1, 4.0, 8.0, 16.0_f64] {
            for past_nyquist in [1.01, 1.2, 1.9] {
                let sound = tone(1 << 15, past_nyquist / speed / 2.0);
                let level = read_level_db(&reader, &sound, speed);
                assert!(level < floor, "{speed} {past_nyquist}: {level} dB");
            }
        }
    }
}

#[test]
fn rate_read_keeps_the_band_below_its_cutoff() {
    let reader = live_reader();
    for speed in [1.5, 2.0, 4.0, 7.3, 16.0_f64] {
        let sound = tone(1 << 15, 0.7 / speed / 2.0);
        let level = read_level_db(&reader, &sound, speed);
        assert!(level.abs() < 1.0e-3, "{speed}: {level} dB");
    }
}

#[test]
fn rate_read_tracks_a_gliding_rate_through_rate_one() {
    const READS: usize = 40_000;
    const CYCLES_PER_SAMPLE: f64 = 0.002;
    let exact = |position: f64| (std::f64::consts::TAU * CYCLES_PER_SAMPLE * position).sin();
    let sound = tone(1 << 22, CYCLES_PER_SAMPLE);
    for (reader, floor) in [(live_reader(), 3.0e-5), (best_reader(), 3.0e-7)] {
        let mut position = 8192.3;
        for read in 0..READS {
            let rate = 0.9 * (16.0_f64 / 0.9).powf(read as f64 / READS as f64);
            let output = f64::from(reader.sample(&Samples(&sound), position, rate));
            assert!((output - exact(position)).abs() < floor, "{rate}");
            position += rate;
        }
    }
}

/// The largest output change between neighbouring rates on a three-octave glide
/// through rate 1, with `reads` rates and a fixed position.
fn largest_rate_step(reader: &RateSincF32, sound: &[f32], reads: usize, snap: bool) -> f32 {
    let rate_at = |read: usize| {
        let glide = 0.9 * 8.0_f64.powf(read as f64 / reads as f64);
        if snap {
            ((glide.log2() * 12.0).ceil() / 12.0).exp2()
        } else {
            glide
        }
    };
    let position = sound.len() as f64 / 2.0 + 0.37;
    (0..reads)
        .map(|read| {
            let before = reader.sample(&Samples(sound), position, rate_at(read));
            let after = reader.sample(&Samples(sound), position, rate_at(read + 1));
            (after - before).abs()
        })
        .fold(0.0, f32::max)
}

#[test]
fn rate_read_change_shrinks_with_the_rate_step() {
    const COARSE: usize = 2048;
    let sound = noise(1 << 14);
    let reader = live_reader();
    let coarse = largest_rate_step(&reader, &sound, COARSE, false);
    let fine = largest_rate_step(&reader, &sound, COARSE * 16, false);
    assert!(fine < coarse / 8.0, "{fine} against {coarse}");
    // Tables chosen by the twelfth of an octave jump however fine the glide, so
    // this measure separates them from a continuous read.
    let snapped = largest_rate_step(&reader, &sound, COARSE * 16, true);
    assert!(snapped > fine * 100.0, "{snapped} against {fine}");
}

#[test]
fn rate_read_returns_whole_samples_at_rate_one_and_below() {
    let sound = noise(256);
    let reader = live_reader();
    for rate in [1.0, 0.5] {
        for (index, sample) in sound.iter().enumerate() {
            let read = reader.sample(&Samples(&sound), index as f64, rate);
            assert_eq!(read.to_bits(), sample.to_bits());
        }
    }
}

#[test]
fn rate_read_stays_within_its_reach() {
    let reader = live_reader();
    for rate in [0.5, 1.03, 3.0, 16.0, 40.0, f64::NAN] {
        let span = Span::new();
        let position = 10_000.37;
        reader.sample(&span, position, rate);
        let (low, high) = span.0.get();
        let reach = reader.reach(rate) as f64;
        assert!(position - low as f64 <= reach, "{rate}");
        assert!(high as f64 - position <= reach, "{rate}");
    }
    assert_eq!(reader.reach(40.0), reader.reach(reader.max_rate()));
    assert_eq!(reader.reach(f64::NAN), reader.reach(reader.max_rate()));
}

#[test]
fn stereo_rate_read_matches_two_mono_reads() {
    let left = noise(4096);
    let right: Vec<f32> = left.iter().rev().copied().collect();
    let reader = live_reader();
    for rate in [0.7, 1.03, 2.5, 16.0] {
        let [a, b] = reader.sample_stereo(&Samples(&left), &Samples(&right), 2048.3, rate);
        assert_eq!(
            a.to_bits(),
            reader.sample(&Samples(&left), 2048.3, rate).to_bits()
        );
        assert_eq!(
            b.to_bits(),
            reader.sample(&Samples(&right), 2048.3, rate).to_bits()
        );
    }
}
