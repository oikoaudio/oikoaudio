//! A minimum-phase counterpart of `RateSincF32`, and the design behind it.

use super::{FRACTION_SCALE, RateSincF32, SampleSourceF32, kaiser_sinc};

/// Below this, relative to the largest magnitude, a magnitude is taken as this, so that its
/// logarithm stays finite at the zeros on the unit circle. It lies 240 dB down, far below any
/// stopband here.
const FLOOR: f64 = 1.0e-12;

/// Transform points for each table entry, at least, before rounding up to a power of two. At 2 the
/// cepstrum folds onto itself and a stopband loses 4 dB; more than 4 gained nothing measured.
/// Doubling it moves a table's entries by about -52 dB of the peak, so every read changes at that
/// level.
const POINTS_PER_ENTRY: usize = 4;

/// The minimum-phase filter of the same length whose magnitude is that of `taps`, by the cepstrum:
/// the logarithm of the magnitude, transformed back, has its anticausal half folded onto its causal
/// one, and the exponential of its transform is the minimum-phase spectrum. Zeros outside the unit
/// circle move to their mirrors inside it and those on it stay, so a stopband is kept and the delay
/// moves to the start. A negative gain at DC comes back positive, and taps that are all zero give
/// NaN.
///
/// `points` is the size of the four transforms, a power of two of at least twice the length; any
/// other panics. It allocates, so run it outside the audio callback.
pub fn minimum_phase(taps: &[f64], points: usize) -> Vec<f64> {
    assert!(points.is_power_of_two() && points >= taps.len() * 2);
    let turns = turns(points);
    let mut spectrum = vec![Complex::ZERO; points];
    for (point, tap) in spectrum.iter_mut().zip(taps) {
        point.re = *tap;
    }
    forward(&mut spectrum, &turns);
    let peak = spectrum
        .iter()
        .map(|point| point.magnitude())
        .fold(0.0, f64::max);
    for point in &mut spectrum {
        *point = Complex {
            re: point.magnitude().max(peak * FLOOR).ln(),
            im: 0.0,
        };
    }
    inverse(&mut spectrum, &turns);
    for (index, point) in spectrum.iter_mut().enumerate() {
        let fold = match index {
            0 => 1.0,
            _ if index < points / 2 => 2.0,
            _ if index == points / 2 => 1.0,
            _ => 0.0,
        };
        *point = Complex {
            re: point.re * fold,
            im: 0.0,
        };
    }
    forward(&mut spectrum, &turns);
    for point in &mut spectrum {
        *point = point.exp();
    }
    inverse(&mut spectrum, &turns);
    spectrum
        .iter()
        .take(taps.len())
        .map(|point| point.re)
        .collect()
}

#[derive(Clone, Copy)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    const ZERO: Self = Self { re: 0.0, im: 0.0 };

    fn magnitude(self) -> f64 {
        self.re.hypot(self.im)
    }

    fn exp(self) -> Self {
        let scale = self.re.exp();
        let (sin, cos) = self.im.sin_cos();
        Self {
            re: scale * cos,
            im: scale * sin,
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }

    fn minus(self, other: Self) -> Self {
        Self {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }

    fn times(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }

    fn conjugate(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }
}

/// The first half of the circle in `points` steps, for `forward` and `inverse`.
fn turns(points: usize) -> Vec<Complex> {
    (0..points / 2)
        .map(|index| {
            let (sin, cos) = (std::f64::consts::TAU * index as f64 / points as f64).sin_cos();
            Complex { re: cos, im: -sin }
        })
        .collect()
}

/// The transform of `points`, whose length is a power of two, left in bit-reversed order.
/// `inverse` takes that order back, so the pair needs no reordering pass.
fn forward(points: &mut [Complex], turns: &[Complex]) {
    let mut span = points.len();
    while span >= 2 {
        let stride = points.len() / span;
        for block in points.chunks_exact_mut(span) {
            let (low, high) = block.split_at_mut(span / 2);
            for (index, (a, b)) in low.iter_mut().zip(high).enumerate() {
                let (sum, difference) = (a.plus(*b), a.minus(*b));
                *a = sum;
                *b = difference.times(turns[index * stride]);
            }
        }
        span /= 2;
    }
}

/// The inverse of `forward`: takes bit-reversed order, gives natural order, and scales.
fn inverse(points: &mut [Complex], turns: &[Complex]) {
    let mut span = 2;
    while span <= points.len() {
        let stride = points.len() / span;
        for block in points.chunks_exact_mut(span) {
            let (low, high) = block.split_at_mut(span / 2);
            for (index, (a, b)) in low.iter_mut().zip(high).enumerate() {
                let turned = b.times(turns[index * stride].conjugate());
                (*a, *b) = (a.plus(turned), a.minus(turned));
            }
        }
        span *= 2;
    }
    let scale = 1.0 / points.len() as f64;
    for point in points {
        point.re *= scale;
        point.im *= scale;
    }
}

/// Band-limited read at any rate that looks only back from its place, so that nothing of a hit
/// comes before it and its ring lies behind it.
///
/// The table is `RateSincF32`'s turned minimum phase, and a read stretches it along the same
/// cutoff curve, so its magnitude is `RateSincF32`'s and a moving rate has no table boundaries to
/// cross. A read is late by `delay`, and is no longer exact at whole positions, at rate 1 as at
/// any other. Reads are pure functions of the source, position and rate. They allocate nothing
/// and take no lock. Construction allocates and runs four large transforms, so build the table
/// outside the audio callback.
pub struct MinimumPhaseRateSincF32 {
    taps: f64,
    phases: f64,
    passband: f64,
    max_rate: f64,
    delay: f64,
    table: Vec<f32>,
}

impl MinimumPhaseRateSincF32 {
    /// Builds the table from the arguments of `RateSincF32::new`, which mean the same here. For a
    /// shorter read, pass fewer `taps` and a lower `passband`: what is cut from a finished kernel
    /// costs more of the band than a kernel designed at that length does.
    pub fn new(taps: usize, phases: usize, max_rate: f64, passband: f64, beta: f64) -> Self {
        assert!(taps >= 4 && taps.is_multiple_of(2));
        assert!(phases >= 2);
        assert!(max_rate.is_finite() && max_rate >= 1.0);
        assert!(passband > 0.0 && passband <= 1.0);
        let entries = taps * phases + 1;
        let linear: Vec<f64> = kaiser_sinc(taps, phases, entries, beta).collect();
        let points = (entries * POINTS_PER_ENTRY).next_power_of_two();
        let mut table = minimum_phase(&linear, points);
        let weight: f64 = table.iter().sum();
        let moment: f64 = table
            .iter()
            .enumerate()
            .map(|(index, entry)| index as f64 * entry)
            .sum();
        // The extra entry lets interpolation at the table's last point read a neighbour.
        table.push(0.0);
        Self {
            taps: taps as f64,
            phases: phases as f64,
            passband,
            max_rate,
            delay: moment / weight / phases as f64,
            table: table.iter().map(|entry| *entry as f32).collect(),
        }
    }

    /// Input samples back from the position that a read at `rate` can touch, that sample included.
    /// `reach(rate) + 1` frames ending at `floor(position)` hold all of a read.
    pub fn reach(&self, rate: f64) -> usize {
        (self.taps / self.cutoff(rate)).ceil() as usize
    }

    /// Rate above which a read stops following the requested rate.
    pub fn max_rate(&self) -> f64 {
        self.max_rate
    }

    /// Input samples by which a read at `rate` is late for the low end of the band, as heard: a
    /// ramp reads as `position - delay(rate)`. Higher in the band it is later, most of all
    /// next to the cutoff.
    pub fn delay(&self, rate: f64) -> f64 {
        self.delay / self.cutoff(rate)
    }

    /// Output samples by which a flat read is late at every rate up to `max_rate`, the delay of a
    /// plain read from about rate 1.06 up. Above the maximum the read stays `flat_delay * max_rate`
    /// input samples late, so in output samples it falls as `max_rate / rate`.
    pub fn flat_delay(&self) -> f64 {
        self.delay / self.passband
    }

    /// Input samples by which a flat read moves its place ahead of `position`: `sample_flat` is
    /// `sample` at `position + flat_lead(rate)`, and reaches from there. It is at most `delay(0.0)`,
    /// at the lowest rates, slightly negative just around rate 1 and 0 from about rate 1.06 up. A
    /// flat read below rate 1 so looks ahead of its position. There it falls by `flat_delay` per
    /// unit of rate, where a plain read's place does not move, so a jump of the rate jumps the
    /// place: ramp a rate fed to a flat read.
    pub fn flat_lead(&self, rate: f64) -> f64 {
        self.delay(rate) - self.flat_delay() * rate.min(self.max_rate)
    }

    /// Reads `source` at `position` for a read head moving at `rate` source samples per output
    /// sample. Only samples at or before `position` count, none before `position - reach(rate)`. A
    /// constant source reads back exactly at every position and rate. A rate above the maximum, or
    /// NaN, reads at the maximum. A negative rate reads as rate 1 with the ring still behind
    /// `position`, so a read moving backward hears it before the hit.
    #[inline]
    pub fn sample<S: SampleSourceF32>(&self, source: &S, position: f64, rate: f64) -> f32 {
        let [output] = self.fold(position, rate, |index| [source.at_f32(index)]);
        output
    }

    /// `sample` for two channels, sharing one set of weights.
    #[inline]
    pub fn sample_stereo<L: SampleSourceF32, R: SampleSourceF32>(
        &self,
        left: &L,
        right: &R,
        position: f64,
        rate: f64,
    ) -> [f32; 2] {
        self.fold(position, rate, |index| {
            [left.at_f32(index), right.at_f32(index)]
        })
    }

    /// `sample` with its place moved so that the read is `flat_delay` output samples late at every
    /// rate up to the maximum, as `sample` is not.
    #[inline]
    pub fn sample_flat<S: SampleSourceF32>(&self, source: &S, position: f64, rate: f64) -> f32 {
        self.sample(source, position + self.flat_lead(rate), rate)
    }

    /// `sample_flat` for two channels, sharing one set of weights.
    #[inline]
    pub fn sample_stereo_flat<L: SampleSourceF32, R: SampleSourceF32>(
        &self,
        left: &L,
        right: &R,
        position: f64,
        rate: f64,
    ) -> [f32; 2] {
        self.sample_stereo(left, right, position + self.flat_lead(rate), rate)
    }

    #[inline]
    fn cutoff(&self, rate: f64) -> f64 {
        RateSincF32::cutoff_curve(rate, self.max_rate, self.passband)
    }

    #[inline]
    fn fold<const CHANNELS: usize>(
        &self,
        position: f64,
        rate: f64,
        frame: impl Fn(i64) -> [f32; CHANNELS],
    ) -> [f32; CHANNELS] {
        let cutoff = self.cutoff(rate);
        let newest = position.floor();
        let oldest = (position - self.taps / cutoff).ceil();
        let step = (cutoff * self.phases * FRACTION_SCALE) as u64;
        let start = (position - newest) * cutoff * self.phases;
        let mut table_position = (start * FRACTION_SCALE) as u64;
        let last_entry = self.table.len() - 2;
        // `f64`: rounding an `f32` sum over thousands of taps rises above a -140 dB stopband.
        let mut sums = [0.0_f64; CHANNELS];
        let mut total_weight = 0.0_f64;
        // Newest first, so a table position's rounding builds up where the weights are smallest.
        for index in (oldest as i64..=newest as i64).rev() {
            let entry = ((table_position >> 32) as usize).min(last_entry);
            let blend = table_position as u32 as f32 / FRACTION_SCALE as f32;
            let (a, b) = (self.table[entry], self.table[entry + 1]);
            let weight = f64::from(a + blend * (b - a));
            total_weight += weight;
            let samples = frame(index);
            for (sum, sample) in sums.iter_mut().zip(samples) {
                *sum += weight * f64::from(sample);
            }
            table_position = table_position.wrapping_add(step);
        }
        // As in `RateSincF32`: no scale ahead of time sums to one at every place and rate.
        let gain = 1.0 / total_weight;
        sums.map(|sum| (sum * gain) as f32)
    }
}

#[cfg(test)]
mod tests;
