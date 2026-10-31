use std::f64::consts::TAU;
use std::sync::OnceLock;

use super::super::RateSincF32;
use super::*;

struct Samples<'a>(&'a [f32]);

impl SampleSourceF32 for Samples<'_> {
    fn at_f32(&self, index: i64) -> f32 {
        self.0[index.clamp(0, self.0.len() as i64 - 1) as usize]
    }
}

/// Records the first and last index a read touches.
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

fn tone(frames: usize, cycles_per_sample: f64) -> Vec<f32> {
    (0..frames)
        .map(|n| (TAU * cycles_per_sample * n as f64).sin() as f32)
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

const LIVE: (usize, usize, f64, f64, f64) = (48, 256, 16.0, 0.87, 9.0);
const BEST: (usize, usize, f64, f64, f64) = (128, 4096, 16.0, 0.919, 16.0);

fn new_reader(
    (taps, phases, max_rate, passband, beta): (usize, usize, f64, f64, f64),
) -> MinimumPhaseRateSincF32 {
    MinimumPhaseRateSincF32::new(taps, phases, max_rate, passband, beta)
}

fn linear_reader(
    (taps, phases, max_rate, passband, beta): (usize, usize, f64, f64, f64),
) -> RateSincF32 {
    RateSincF32::new(taps, phases, max_rate, passband, beta)
}

fn live_reader() -> &'static MinimumPhaseRateSincF32 {
    static READER: OnceLock<MinimumPhaseRateSincF32> = OnceLock::new();
    READER.get_or_init(|| new_reader(LIVE))
}

/// The Best table takes four transforms of 2^22 points, so the tests share one.
fn best_reader() -> &'static MinimumPhaseRateSincF32 {
    static READER: OnceLock<MinimumPhaseRateSincF32> = OnceLock::new();
    READER.get_or_init(|| new_reader(BEST))
}

/// The shape for a read of 36 taps, which `a_shorter_kernel_is_flat_up_to_its_band` holds to.
fn short_reader() -> &'static MinimumPhaseRateSincF32 {
    static READER: OnceLock<MinimumPhaseRateSincF32> = OnceLock::new();
    READER.get_or_init(|| new_reader((36, 256, 16.0, 0.83, 9.0)))
}

fn level_db(read: impl Fn(&Samples, f64, f64) -> f32, sound: &[f32], speed: f64) -> f64 {
    const READS: usize = 2000;
    let power = (0..READS)
        .map(|index| {
            let position = sound.len() as f64 / 2.0 + index as f64 * speed + 0.37;
            f64::from(read(&Samples(sound), position, speed)).powi(2)
        })
        .sum::<f64>()
        / READS as f64;
    10.0 * (power * 2.0).max(1.0e-30).log10()
}

#[test]
fn transforms_match_a_direct_sum_and_invert() {
    const SIZE: usize = 32;
    let sound = noise(SIZE * 2);
    let input: Vec<Complex> = (0..SIZE)
        .map(|index| Complex {
            re: f64::from(sound[index * 2]),
            im: f64::from(sound[index * 2 + 1]),
        })
        .collect();
    let turns = turns(SIZE);
    let mut points = input.clone();
    forward(&mut points, &turns);
    for (bin, point) in points.iter().enumerate() {
        let frequency = bin.reverse_bits() >> (usize::BITS - SIZE.ilog2());
        let direct = input
            .iter()
            .enumerate()
            .fold(Complex::ZERO, |sum, (index, value)| {
                let (sin, cos) = (TAU * (frequency * index) as f64 / SIZE as f64).sin_cos();
                sum.plus(value.times(Complex { re: cos, im: -sin }))
            });
        assert!(direct.minus(*point).magnitude() < 1.0e-12, "{bin}");
    }
    inverse(&mut points, &turns);
    for (point, value) in points.iter().zip(&input) {
        assert!(point.minus(*value).magnitude() < 1.0e-14);
    }
}

fn response(taps: &[f64], cycles_per_tap: f64) -> f64 {
    taps.iter()
        .enumerate()
        .fold(Complex::ZERO, |sum, (index, tap)| {
            let (sin, cos) = (TAU * cycles_per_tap * index as f64).sin_cos();
            sum.plus(Complex {
                re: tap * cos,
                im: -tap * sin,
            })
        })
        .magnitude()
}

#[test]
fn minimum_phase_keeps_the_gain_and_magnitude_and_moves_the_energy_to_the_start() {
    const TAPS: usize = 151;
    let centre = (TAPS / 2) as f64;
    let linear: Vec<f64> = (0..TAPS)
        .map(|index| {
            let from_centre = (index as f64 - centre) / (centre + 1.0);
            let lag = (index as f64 - centre) * 0.4;
            let sinc = if lag == 0.0 {
                1.0
            } else {
                (std::f64::consts::PI * lag).sin() / (std::f64::consts::PI * lag)
            };
            sinc * (std::f64::consts::FRAC_PI_2 * from_centre).cos().powi(2)
        })
        .collect();
    let minimum = minimum_phase(&linear, 1 << 14);
    assert_eq!(minimum.len(), TAPS);
    for bin in 0..200 {
        let frequency = bin as f64 / 400.0;
        let (a, b) = (response(&linear, frequency), response(&minimum, frequency));
        assert!(
            (a - b).abs() < 1.0e-3 * a.max(1.0e-3),
            "{frequency}: {a} {b}"
        );
    }
    let first_half = |taps: &[f64]| taps[..TAPS / 2].iter().map(|tap| tap * tap).sum::<f64>();
    let whole: f64 = minimum.iter().map(|tap| tap * tap).sum();
    assert!(first_half(&minimum) > 0.999 * whole);
    assert!(first_half(&linear) < 0.6 * whole);
}

#[test]
fn an_impulse_ahead_of_a_read_does_not_reach_it() {
    const HIT: i64 = 1000;
    let mut sound = vec![0.0_f32; 2048];
    sound[HIT as usize] = 1.0;
    let rates = [0.0625, 0.25, 0.9, 1.0, 1.03, 1.5, 2.0, 7.3, 16.0];
    for reader in [live_reader(), best_reader(), short_reader()] {
        for rate in rates {
            for place in 0..400 {
                let position = HIT as f64 - place as f64 * 0.137 - 1.0e-9;
                let read = reader.sample(&Samples(&sound), position, rate);
                assert_eq!(read, 0.0, "{rate} {position}");
            }
            let after = (0..400)
                .map(|place| {
                    let position = HIT as f64 + place as f64 * 0.137;
                    reader.sample(&Samples(&sound), position, rate).abs()
                })
                .fold(0.0, f32::max);
            assert!(after > 0.01 / rate.max(1.0) as f32, "{rate}");
        }
    }
}

#[test]
fn magnitude_is_rate_sincs_in_the_band() {
    let live = linear_reader(LIVE);
    let best = linear_reader(BEST);
    let pairs: [(&MinimumPhaseRateSincF32, &RateSincF32); 2] =
        [(live_reader(), &live), (best_reader(), &best)];
    for (minimum, linear) in pairs {
        for speed in [0.5, 1.0, 1.03, 1.5, 2.0, 4.0, 7.3, 16.0_f64] {
            for output_band in [0.05, 0.2, 0.4, 0.6, 0.7, 0.8] {
                let sound = tone(1 << 16, output_band / speed.max(1.0) / 2.0);
                let a = level_db(|s, p, r| minimum.sample(s, p, r), &sound, speed);
                let b = level_db(|s, p, r| linear.sample(s, p, r), &sound, speed);
                assert!((a - b).abs() < 0.01, "{speed} {output_band}: {a} {b}");
            }
        }
    }
}

#[test]
fn stopband_matches_rate_sincs() {
    let live = linear_reader(LIVE);
    let best = linear_reader(BEST);
    let cases: [(&MinimumPhaseRateSincF32, &RateSincF32, f64, f64); 2] = [
        (live_reader(), &live, -91.0, 3.0),
        (best_reader(), &best, -146.0, 6.0),
    ];
    for (minimum, linear, floor, margin) in cases {
        for speed in [2.0, 3.1, 4.0, 8.0, 16.0_f64] {
            let near = [1.003, 1.01, 1.02, 1.2, 1.25, 1.5, 1.9];
            let far = [4.0, 12.0].into_iter().filter(|_| speed == 16.0);
            let (mut worst, mut reference) = (f64::MIN, f64::MIN);
            for past_nyquist in near.into_iter().chain(far) {
                // Long enough that a read of `linear`, ahead of its place too, stays inside it.
                let sound = tone(1 << 17, past_nyquist / speed / 2.0);
                let a = level_db(|s, p, r| minimum.sample(s, p, r), &sound, speed);
                let b = level_db(|s, p, r| linear.sample(s, p, r), &sound, speed);
                assert!(a < floor, "{speed} {past_nyquist}: {a} dB");
                worst = worst.max(a);
                reference = reference.max(b);
            }
            assert!(worst < reference + margin, "{speed}: {worst} {reference}");
        }
    }
}

#[test]
fn a_read_keeps_a_constant_at_every_place_and_rate() {
    let sound = vec![1.0_f32; 1 << 14];
    for reader in [live_reader(), best_reader(), short_reader()] {
        for rate in [0.5, 1.0, 1.027, 1.03, 1.2, 2.0, 7.3, 16.0] {
            for place in 0..500 {
                let position = 8192.0 + place as f64 * 0.618_033_988_75;
                let read = reader.sample(&Samples(&sound), position, rate);
                assert_eq!(read, 1.0, "{rate} {position}");
            }
        }
    }
}

#[test]
fn a_read_keeps_a_constant_past_the_ends_and_at_odd_positions() {
    let sound = vec![1.0_f32; 4096];
    let positions = [
        -100.5,
        -0.5,
        0.0,
        5000.25,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    for reader in [live_reader(), short_reader()] {
        for position in positions {
            let read = reader.sample(&Samples(&sound), position, 1.0);
            assert_eq!(read, 1.0, "{position}");
        }
    }
}

const RATES: [f64; 9] = [0.25, 0.5, 1.0, 1.027, 1.03, 1.2, 2.0, 7.3, 16.0];

/// A ramp's read, against its value at `position`, by reads through `read`.
fn lag_on_a_ramp(read: impl Fn(&Samples, f64, f64) -> f32, rate: f64, check: impl Fn(f64, f64)) {
    const SLOPE: f64 = 1.0 / 8192.0;
    let sound: Vec<f32> = (0..1 << 14).map(|n| (n as f64 * SLOPE) as f32).collect();
    for place in 0..500 {
        let position = 8192.0 + place as f64 * 0.618_033_988_75;
        let value = f64::from(read(&Samples(&sound), position, rate));
        check(position, position - value / SLOPE);
    }
}

#[test]
fn a_ramp_reads_late_by_the_delay_at_every_rate() {
    for reader in [live_reader(), best_reader(), short_reader()] {
        for rate in RATES {
            lag_on_a_ramp(
                |s, p, r| reader.sample(s, p, r),
                rate,
                |position, lag| {
                    let error = (lag - reader.delay(rate)).abs();
                    assert!(error < 2.0e-3, "{rate} {position}: {lag}");
                },
            );
        }
    }
}

#[test]
fn delay_stands_at_the_start_of_the_kernel() {
    let reader = live_reader();
    assert!((reader.delay(1.0) - 2.65).abs() < 0.03);
    assert!((best_reader().delay(1.0) - 4.16).abs() < 0.03);
    assert!((reader.delay(2.0) / 2.0 - reader.flat_delay()).abs() < 1.0e-12);
    assert!((reader.delay(0.25) - reader.delay(1.0)).abs() < 1.0e-12);
}

#[test]
fn a_flat_read_is_equally_late_at_every_rate() {
    for reader in [live_reader(), best_reader(), short_reader()] {
        for rate in RATES.into_iter().chain([40.0]) {
            lag_on_a_ramp(
                |s, p, r| reader.sample_flat(s, p, r),
                rate,
                |position, lag| {
                    let expected = reader.flat_delay() * rate.min(reader.max_rate());
                    assert!((lag - expected).abs() < 2.0e-3, "{rate} {position}: {lag}");
                },
            );
        }
    }
}

#[test]
fn flat_lead_keeps_to_its_bounds() {
    let reader = live_reader();
    assert_eq!(reader.flat_lead(reader.max_rate()), reader.flat_lead(40.0));
    assert_eq!(reader.flat_lead(f64::NAN), reader.flat_lead(40.0));
    assert!(reader.flat_lead(2.0).abs() < 1.0e-12);
    assert!(reader.flat_lead(16.0).abs() < 1.0e-12);
    assert!((reader.flat_lead(0.0) - reader.delay(0.0)).abs() < 1.0e-12);
    for rate in RATES {
        assert!(reader.flat_lead(rate) <= reader.delay(0.0));
        assert!(reader.flat_lead(rate) > -0.5, "{rate}");
    }
}

#[test]
fn a_read_tracks_a_gliding_rate_through_rate_one() {
    const READS: usize = 40_000;
    const CYCLES_PER_SAMPLE: f64 = 0.002;
    let exact = |position: f64| (TAU * CYCLES_PER_SAMPLE * position).sin();
    let sound = tone(1 << 22, CYCLES_PER_SAMPLE);
    // `delay` is the delay at the low end. The phase's bend leaves an error as the cube of the
    // tone's place in the band, -58 dB at rate 16, where it stands at 1.5 kHz of 48. No moving of
    // the rate adds to it, so below that the floor is the read's own, -107 dB.
    let floor = |rate: f64| 4.0e-6 + 3.5e-7 * rate.powi(3);
    for reader in [live_reader(), best_reader()] {
        let mut position = 8192.3;
        for read in 0..READS {
            let rate = 0.9 * (16.0_f64 / 0.9).powf(read as f64 / READS as f64);
            let output = f64::from(reader.sample(&Samples(&sound), position, rate));
            let error = (output - exact(position - reader.delay(rate))).abs();
            assert!(error < floor(rate), "{rate}: {error}");
            position += rate;
        }
    }
}

/// The largest output change between neighbouring rates on a three-octave glide through rate 1,
/// with `reads` rates and a fixed position.
fn largest_rate_step(
    read: impl Fn(&Samples, f64, f64) -> f32,
    sound: &[f32],
    position: f64,
    reads: usize,
) -> f32 {
    let rate_at = |read: usize| 0.9 * 8.0_f64.powf(read as f64 / reads as f64);
    let outputs: Vec<f32> = (0..=reads)
        .map(|index| read(&Samples(sound), position, rate_at(index)))
        .collect();
    outputs
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f32::max)
}

/// A read that follows the rate smoothly changes 16 times less for 16 times smaller rate steps. A
/// step in it does not shrink, so it holds the ratio down.
fn assert_change_shrinks(read: impl Fn(&Samples, f64, f64) -> f32, sound: &[f32], position: f64) {
    const COARSE: usize = 2048;
    let coarse = largest_rate_step(&read, sound, position, COARSE);
    let fine = largest_rate_step(&read, sound, position, COARSE * 16);
    assert!(fine < coarse / 15.0, "{position}: {fine} of {coarse}");
}

#[test]
fn a_reads_change_shrinks_with_the_rate_step() {
    let sound = noise(1 << 14);
    let middle = sound.len() as f64 / 2.0;
    for reader in [live_reader(), best_reader()] {
        for offset in [0.37, 1.61, 100.05, 2000.8, -3000.2] {
            let plain = |s: &Samples, p: f64, r: f64| reader.sample(s, p, r);
            assert_change_shrinks(plain, &sound, middle + offset);
            let flat = |s: &Samples, p: f64, r: f64| reader.sample_flat(s, p, r);
            assert_change_shrinks(flat, &sound, middle + offset);
        }
    }
}

#[test]
fn a_read_looks_only_back_within_its_reach() {
    for reader in [live_reader(), short_reader()] {
        for rate in [0.5, 1.0, 1.03, 3.0, 16.0, 40.0, f64::NAN] {
            for position in [10_000.37, 10_000.0, 0.0, -3.5] {
                let span = Span::new();
                reader.sample(&span, position, rate);
                let (low, high) = span.0.get();
                assert_eq!(high, position.floor() as i64, "{rate} {position}");
                let back = position - low as f64;
                assert!(back <= reader.reach(rate) as f64, "{rate} {position}");
                let frames = reader.reach(rate) as i64 + 1;
                assert!(high - low < frames, "{rate} {position}");
            }
        }
        assert_eq!(reader.reach(40.0), reader.reach(reader.max_rate()));
        assert_eq!(reader.reach(f64::NAN), reader.reach(reader.max_rate()));
        // At a whole position and rate 1 the frame at the reach is read too.
        let span = Span::new();
        reader.sample(&span, 10_000.0, 1.0);
        assert_eq!(span.0.get().0, 10_000 - reader.reach(1.0) as i64);
    }
}

#[test]
fn stereo_reads_match_two_mono_reads() {
    let left = noise(4096);
    let right: Vec<f32> = left.iter().rev().copied().collect();
    let reader = live_reader();
    let (left, right) = (Samples(&left), Samples(&right));
    for rate in [0.7, 1.03, 2.5, 16.0] {
        let [a, b] = reader.sample_stereo(&left, &right, 2048.3, rate);
        assert_eq!(a.to_bits(), reader.sample(&left, 2048.3, rate).to_bits());
        assert_eq!(b.to_bits(), reader.sample(&right, 2048.3, rate).to_bits());
        let [a, b] = reader.sample_stereo_flat(&left, &right, 2048.3, rate);
        assert_eq!(
            a.to_bits(),
            reader.sample_flat(&left, 2048.3, rate).to_bits()
        );
        assert_eq!(
            b.to_bits(),
            reader.sample_flat(&right, 2048.3, rate).to_bits()
        );
    }
}

#[test]
fn a_shorter_kernel_is_flat_up_to_its_band_and_keeps_the_stopband() {
    let reader = short_reader();
    assert_eq!(reader.reach(1.0), 36);
    for speed in [2.0, 4.0, 16.0_f64] {
        for past_nyquist in [1.003, 1.01, 1.02, 1.2, 1.9] {
            let sound = tone(1 << 16, past_nyquist / speed / 2.0);
            let level = level_db(|s, p, r| reader.sample(s, p, r), &sound[..], speed);
            assert!(level < -90.0, "{speed} {past_nyquist}: {level} dB");
        }
        let sound = tone(1 << 16, 0.7 / speed / 2.0);
        let level = level_db(|s, p, r| reader.sample(s, p, r), &sound[..], speed);
        assert!(level.abs() < 0.02, "{speed}: {level} dB");
    }
}
