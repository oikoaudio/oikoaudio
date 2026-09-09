//! Host-independent mask generation for Oiko Weft.

pub mod particles;
/// One stored gain value for every bin in the maximum 16,384-sample RFFT.
/// Smaller processing resolutions land on exact subsets of this master mask.
pub mod processing;

pub const MANUAL_MASK_POINTS: usize = 8193;
pub const MIDI_NOTES: usize = 128;
pub const MAX_PARTIALS: usize = 24;
pub const MIN_DISPLAY_FREQUENCY_HZ: f32 = 20.0;
pub const MANUAL_CURVE_MUTE_DB: f32 = -144.0;
const DISPLAY_OCTAVES: f32 = 9.965_784;
const MOTION_COMPENSATION_SAMPLES: usize = 256;
const MAX_MOTION_COMPENSATION_DB: f32 = 12.0;

#[derive(Clone, Copy, Debug)]
pub struct MaskVoice {
    pub note: u8,
    pub level: f32,
    pub tuning_semitones: f32,
    pub pressure: f32,
    pub timbre: f32,
    /// Base strength (including velocity), bounded to openness before expression.
    pub volume_gain: f32,
    /// Linear gain of the note-opened component, independent of the background floor.
    pub expression_gain: f32,
    /// Stereo placement, -1 left through 0 center to +1 right. Ignored in mono.
    pub pan: f32,
}

impl Default for MaskVoice {
    fn default() -> Self {
        Self {
            note: 0,
            level: 0.0,
            tuning_semitones: 0.0,
            pressure: 1.0,
            timbre: 0.5,
            volume_gain: 1.0,
            expression_gain: 1.0,
            pan: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MotionShape {
    #[default]
    Ripple,
    Harmonic,
    Drift,
    Scan,
    Notch,
    Saw,
    Sprinkle,
    Cloud,
}

impl MotionShape {
    pub fn is_particle(self) -> bool {
        matches!(self, Self::Sprinkle | Self::Cloud)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MotionConfig {
    pub shape: MotionShape,
    pub depth_db: f32,
    pub phase: f32,
    pub size_octaves: f32,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            shape: MotionShape::Ripple,
            depth_db: 0.0,
            phase: 0.0,
            size_octaves: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MaskConfig {
    pub sample_rate: f32,
    pub fft_size: usize,
    /// Attenuation below the drawn ceiling away from held notes. Zero leaves
    /// the background unchanged while the additive note lift remains active.
    pub note_depth_db: f32,
    pub note_layer_enabled: bool,
    pub width_cents: f32,
    pub partials: usize,
    pub harmonic_rolloff_db_per_octave: f32,
    pub motion: MotionConfig,
}

const GAUSSIAN_LUT_STEPS: usize = 1024;
const GAUSSIAN_MAX_RELATIVE_SQUARED: f32 = 16.0;

/// Precomputed values used while rebuilding a spectral mask. Preparing this
/// outside the audio callback removes the logarithm and exponential calls
/// from the hot bins x partials loop.
pub struct MaskWorkspace {
    bin_log2_hz: Vec<f32>,
    gaussian: [f32; GAUSSIAN_LUT_STEPS + 1],
    sample_rate: f32,
    fft_size: usize,
}

impl Default for MaskWorkspace {
    fn default() -> Self {
        let gaussian = std::array::from_fn(|index| {
            let relative_squared =
                index as f32 * GAUSSIAN_MAX_RELATIVE_SQUARED / GAUSSIAN_LUT_STEPS as f32;
            (-0.5 * relative_squared).exp()
        });
        Self {
            bin_log2_hz: Vec::new(),
            gaussian,
            sample_rate: 0.0,
            fft_size: 0,
        }
    }
}

impl MaskWorkspace {
    /// Refresh the frequency table after a sample-rate or FFT-size change.
    /// This may resize storage and must not be called from the steady-state
    /// real-time path.
    pub fn prepare(&mut self, sample_rate: f32, fft_size: usize) {
        if self.sample_rate == sample_rate && self.fft_size == fft_size {
            return;
        }
        let bin_hz = sample_rate / fft_size as f32;
        self.bin_log2_hz.resize(fft_size / 2 + 1, 0.0);
        for (bin, log2_hz) in self.bin_log2_hz.iter_mut().enumerate().skip(1) {
            *log2_hz = (bin as f32 * bin_hz).log2();
        }
        self.sample_rate = sample_rate;
        self.fft_size = fft_size;
    }

    #[inline]
    fn gaussian(&self, relative_squared: f32) -> f32 {
        let position = relative_squared.clamp(0.0, GAUSSIAN_MAX_RELATIVE_SQUARED)
            * (GAUSSIAN_LUT_STEPS as f32 / GAUSSIAN_MAX_RELATIVE_SQUARED);
        let left = position as usize;
        let right = (left + 1).min(GAUSSIAN_LUT_STEPS);
        let fraction = position - left as f32;
        self.gaussian[left] + (self.gaussian[right] - self.gaussian[left]) * fraction
    }
}

impl Default for MaskConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000.0,
            fft_size: 8192,
            note_depth_db: 0.0,
            note_layer_enabled: true,
            width_cents: 100.0,
            partials: 1,
            harmonic_rolloff_db_per_octave: 6.0,
            motion: MotionConfig::default(),
        }
    }
}

use oiko_dsp::{db_to_gain, note_frequency_with_tuning};

/// Automatic gain applied only to the note-opened component. Narrow skirts
/// receive up to +6 dB even at zero Note Depth, making notes additive before
/// the control begins attenuating the surrounding spectrum. Wide skirts need
/// less compensation, so the lift tapers smoothly above 100 cents and reaches
/// unity at 1200 cents.
#[inline]
pub fn note_emphasis_gain(width_cents: f32) -> f32 {
    let width_position = if width_cents <= 100.0 {
        0.0
    } else {
        (width_cents / 100.0).log2() / 12.0_f32.log2()
    }
    .clamp(0.0, 1.0);
    let narrowness = 1.0 - smoothstep(width_position);
    db_to_gain(6.0 * narrowness)
}

/// Gain of the complete note layer at one bin. Closed regions follow Note
/// Depth while fully open note centres retain their width-compensated lift.
#[inline]
pub fn note_gate_gain(note_depth_db: f32, width_cents: f32, openness: f32) -> f32 {
    let floor = db_to_gain(-note_depth_db.max(0.0));
    floor + (note_emphasis_gain(width_cents) - floor) * openness.clamp(0.0, 1.0)
}

#[inline]
fn smoothstep(position: f32) -> f32 {
    position * position * (3.0 - 2.0 * position)
}

/// Advance a bank of note envelopes once per STFT frame.
pub fn advance_note_envelopes(
    levels: &mut [f32; MIDI_NOTES],
    held: &[bool; MIDI_NOTES],
    elapsed_seconds: f32,
    fade_ms: f32,
) {
    let alpha = smoothing_alpha(elapsed_seconds, fade_ms * 0.001);

    for (note, level) in levels.iter_mut().enumerate() {
        let target = if held[note] { 1.0 } else { 0.0 };
        *level += (target - *level) * alpha;
        if level.abs() < 1.0e-6 {
            *level = 0.0;
        }
    }
}

/// Build the real, non-negative gain mask for an RFFT spectrum.
///
/// `output.len()` is expected to be `fft_size / 2 + 1`. No allocation is
/// performed, making the function safe to call from the audio thread.
pub fn build_mask(
    output: &mut [f32],
    manual_mask_db: &[f32; MANUAL_MASK_POINTS],
    note_levels: &[f32; MIDI_NOTES],
    config: MaskConfig,
) {
    let mut voices = [MaskVoice::default(); MIDI_NOTES];
    for (note, (voice, level)) in voices.iter_mut().zip(note_levels).enumerate() {
        voice.note = note as u8;
        voice.level = *level;
    }
    build_mask_with_voices(output, manual_mask_db, &voices, config);
}

pub fn build_mask_with_voices(
    output: &mut [f32],
    manual_mask_db: &[f32; MANUAL_MASK_POINTS],
    voices: &[MaskVoice],
    config: MaskConfig,
) {
    build_mask_with_voices_impl(output, manual_mask_db, voices, config, None, None);
}

/// Workspace-backed mask builder for the plug-in's real-time path.
pub fn build_mask_with_voices_precomputed(
    output: &mut [f32],
    manual_mask_db: &[f32; MANUAL_MASK_POINTS],
    voices: &[MaskVoice],
    config: MaskConfig,
    workspace: &MaskWorkspace,
) {
    debug_assert_eq!(workspace.sample_rate, config.sample_rate);
    debug_assert_eq!(workspace.fft_size, config.fft_size);
    build_mask_with_voices_impl(
        output,
        manual_mask_db,
        voices,
        config,
        Some(workspace),
        None,
    );
}

/// Stereo note contributions use a constant-power pan law with unity at center.
/// The manual curve, motion and closed-note floor remain shared. Overlapping
/// contributions retain the existing maximum rule independently on each side.
pub fn build_stereo_masks_with_voices_precomputed(
    left: &mut [f32],
    right: &mut [f32],
    manual_mask_db: &[f32; MANUAL_MASK_POINTS],
    voices: &[MaskVoice],
    config: MaskConfig,
    workspace: &MaskWorkspace,
) {
    build_mask_with_voices_impl(
        left,
        manual_mask_db,
        voices,
        config,
        Some(workspace),
        Some(0),
    );
    if voices
        .iter()
        .all(|voice| voice.level <= 1.0e-5 || voice.pan == 0.0)
    {
        // Preserve the original single-mask cost for the common centered case.
        right.copy_from_slice(left);
        return;
    }
    build_mask_with_voices_impl(
        right,
        manual_mask_db,
        voices,
        config,
        Some(workspace),
        Some(1),
    );
}

fn expression_channel_gain(voice: &MaskVoice, channel: Option<usize>) -> f32 {
    let pan = if voice.pan.is_finite() {
        voice.pan.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let gain = if voice.expression_gain.is_finite() {
        voice.expression_gain.clamp(0.0, 4.0)
    } else {
        1.0
    };
    let balance = match channel {
        _ if pan == 0.0 => 1.0,
        Some(0) if pan == 1.0 => 0.0,
        Some(1) if pan == -1.0 => 0.0,
        Some(0) => ((pan + 1.0) * std::f32::consts::FRAC_PI_4).cos() * std::f32::consts::SQRT_2,
        Some(1) => ((pan + 1.0) * std::f32::consts::FRAC_PI_4).sin() * std::f32::consts::SQRT_2,
        _ => 1.0,
    };
    gain * balance
}

fn build_mask_with_voices_impl(
    output: &mut [f32],
    manual_mask_db: &[f32; MANUAL_MASK_POINTS],
    voices: &[MaskVoice],
    config: MaskConfig,
    workspace: Option<&MaskWorkspace>,
    channel: Option<usize>,
) {
    debug_assert_eq!(output.len(), config.fft_size / 2 + 1);
    let nyquist = config.sample_rate * 0.5;
    let bin_hz = config.sample_rate / config.fft_size as f32;
    let width = config.width_cents.max(1.0);
    let partials = config.partials.max(1);
    let motion_enabled = config.motion.depth_db > 0.001;
    let motion_compensation = motion_compensation_gain(config.motion);

    // First use the output buffer as an openness scratch buffer. Updating
    // only the bins within four standard deviations of each lobe avoids a
    // full bins × notes × partials scan.
    output.fill(0.0);
    if config.note_layer_enabled && voices.iter().any(|voice| voice.level > 1.0e-5) {
        let radius_octaves = 4.0 * width / 1200.0;
        let lower_ratio = 2.0_f32.powf(-radius_octaves);
        let upper_ratio = 2.0_f32.powf(radius_octaves);
        for voice in voices {
            if voice.level <= 1.0e-5 {
                continue;
            }

            let expression_gain = expression_channel_gain(voice, channel);
            let pressure_gain = 0.35 + 0.65 * voice.pressure.clamp(0.0, 1.0);
            let note_level = voice.level * voice.volume_gain.max(0.0) * pressure_gain;
            let timbre_rolloff = (config.harmonic_rolloff_db_per_octave
                + (0.5 - voice.timbre.clamp(0.0, 1.0)) * 24.0)
                .clamp(0.0, 48.0);
            let fundamental = note_frequency_with_tuning(voice.note, voice.tuning_semitones);
            if let Some(workspace) = workspace
                && rasterize_compact_harmonic_envelope(
                    output,
                    workspace,
                    fundamental,
                    note_level,
                    expression_gain,
                    timbre_rolloff,
                    partials.min(MAX_PARTIALS),
                    width,
                    bin_hz,
                    nyquist,
                    lower_ratio,
                    upper_ratio,
                )
            {
                continue;
            }
            for partial in 1..=partials.min(MAX_PARTIALS) {
                let center = fundamental * partial as f32;
                if center > nyquist {
                    break;
                }
                let harmonic_gain = db_to_gain(-timbre_rolloff * (partial as f32).log2());
                let first_bin = ((center * lower_ratio) / bin_hz).floor().max(1.0) as usize;
                let last_bin = (((center * upper_ratio) / bin_hz).ceil() as usize)
                    .min(output.len().saturating_sub(1));
                // The musical center will almost always fall between two
                // linearly spaced FFT bins. Normalize the sampled Gaussian
                // against the closest usable bin so fractional placement
                // cannot make an otherwise identical note arbitrarily weak.
                let peak_bin = closest_bin_in_cents(center, bin_hz, output.len() - 1);
                let center_log2 = center.log2();
                let peak_normalized = if let Some(workspace) = workspace {
                    1200.0 * (workspace.bin_log2_hz[peak_bin] - center_log2) / width
                } else {
                    let peak_frequency = peak_bin as f32 * bin_hz;
                    1200.0 * (peak_frequency / center).log2() / width
                };
                for (bin, openness) in output
                    .iter_mut()
                    .enumerate()
                    .take(last_bin + 1)
                    .skip(first_bin)
                {
                    let normalized = if let Some(workspace) = workspace {
                        1200.0 * (workspace.bin_log2_hz[bin] - center_log2) / width
                    } else {
                        let frequency = bin as f32 * bin_hz;
                        1200.0 * (frequency / center).log2() / width
                    };
                    let relative_squared =
                        (normalized * normalized - peak_normalized * peak_normalized).max(0.0);
                    let bell = workspace.map_or_else(
                        || (-0.5 * relative_squared).exp(),
                        |workspace| workspace.gaussian(relative_squared),
                    );
                    *openness = openness
                        .max((note_level * harmonic_gain * bell).clamp(0.0, 1.0) * expression_gain);
                }
            }
        }
    }

    for (bin, gain) in output.iter_mut().enumerate() {
        let manual = manual_gain_at_bin(manual_mask_db, bin, config.fft_size);
        let motion_gain = if motion_enabled {
            let frequency = bin as f32 * bin_hz;
            db_to_gain(-motion_attenuation_db(frequency, config.motion)) * motion_compensation
        } else {
            1.0
        };
        let openness = *gain;
        let note_gate = if config.note_layer_enabled {
            let floor = db_to_gain(-config.note_depth_db.max(0.0));
            floor + (note_emphasis_gain(config.width_cents) - floor) * openness
        } else {
            1.0
        };
        *gain = manual * motion_gain * note_gate;
    }
}

#[allow(clippy::too_many_arguments)]
fn rasterize_compact_harmonic_envelope(
    output: &mut [f32],
    workspace: &MaskWorkspace,
    fundamental: f32,
    note_level: f32,
    expression_gain: f32,
    timbre_rolloff: f32,
    requested_partials: usize,
    width_cents: f32,
    bin_hz: f32,
    nyquist: f32,
    lower_ratio: f32,
    upper_ratio: f32,
) -> bool {
    let active_partials = requested_partials
        .min((nyquist / fundamental).floor().max(0.0) as usize)
        .min(MAX_PARTIALS);
    if active_partials < 2 {
        return false;
    }

    let first_bin = ((fundamental * lower_ratio) / bin_hz).floor().max(1.0) as usize;
    let last_bin = (((fundamental * active_partials as f32 * upper_ratio) / bin_hz).ceil()
        as usize)
        .min(output.len().saturating_sub(1));
    let compact_cost = last_bin.saturating_sub(first_bin) * 2;
    let legacy_cost = (1..=active_partials)
        .map(|partial| {
            let center = fundamental * partial as f32;
            let first = ((center * lower_ratio) / bin_hz).floor().max(1.0) as usize;
            let last = (((center * upper_ratio) / bin_hz).ceil() as usize)
                .min(output.len().saturating_sub(1));
            last.saturating_sub(first)
        })
        .sum::<usize>();
    if compact_cost >= legacy_cost {
        return false;
    }

    let mut center_log2 = [0.0_f32; MAX_PARTIALS];
    let mut peak_normalized_squared = [0.0_f32; MAX_PARTIALS];
    let mut harmonic_gain = [0.0_f32; MAX_PARTIALS];
    for partial in 1..=active_partials {
        let index = partial - 1;
        let center = fundamental * partial as f32;
        center_log2[index] = center.log2();
        harmonic_gain[index] = db_to_gain(-timbre_rolloff * (partial as f32).log2());
        let peak_bin = closest_bin_in_cents(center, bin_hz, output.len() - 1);
        let peak_normalized =
            1200.0 * (workspace.bin_log2_hz[peak_bin] - center_log2[index]) / width_cents;
        peak_normalized_squared[index] = peak_normalized * peak_normalized;
    }

    // In log-frequency space every harmonic is an equal-width parabola. Its
    // continuous maximum, including partial rolloff, tells us which two
    // neighbouring integer harmonics can win at a bin. This replaces many
    // overlapping partial passes with one bounded walk over the spectrum.
    let sigma_octaves = width_cents / 1200.0;
    let natural_rolloff_per_octave = timbre_rolloff * std::f32::consts::LN_10 / 20.0;
    let rolloff_shift = 2.0_f32.powf(-natural_rolloff_per_octave * sigma_octaves * sigma_octaves);
    let minimum_supported_ratio = 2.0_f32.powf(-4.0 * sigma_octaves);
    let bin_to_partial = bin_hz / fundamental;

    for (bin, openness) in output
        .iter_mut()
        .enumerate()
        .take(last_bin + 1)
        .skip(first_bin)
    {
        let frequency_ratio = bin as f32 * bin_to_partial;
        let continuous_partial = (frequency_ratio * rolloff_shift)
            .max(frequency_ratio * minimum_supported_ratio)
            .clamp(1.0, active_partials as f32);
        let lower = continuous_partial.floor() as usize;
        let upper = (lower + 1).min(active_partials);
        let mut best = 0.0_f32;
        for partial in [lower, upper] {
            let index = partial - 1;
            let normalized =
                1200.0 * (workspace.bin_log2_hz[bin] - center_log2[index]) / width_cents;
            if normalized.abs() > 4.0 {
                continue;
            }
            let relative_squared = (normalized * normalized - peak_normalized_squared[index])
                .clamp(0.0, GAUSSIAN_MAX_RELATIVE_SQUARED);
            best = best.max(harmonic_gain[index] * workspace.gaussian(relative_squared));
        }
        *openness = openness.max((note_level * best).clamp(0.0, 1.0) * expression_gain);
    }
    true
}

/// Returns the moving field's openness at a frequency. `1.0` is the crest,
/// where the drawn ceiling is unchanged, and `0.0` receives the full motion
/// attenuation. This is shared by the DSP and editor visualization.
pub fn motion_openness(frequency: f32, config: MotionConfig) -> f32 {
    if config.depth_db <= 0.001 || frequency < MIN_DISPLAY_FREQUENCY_HZ {
        return 1.0;
    }
    let x = (frequency / MIN_DISPLAY_FREQUENCY_HZ).log2();
    let size = config.size_octaves.max(0.05);
    let phase = config.phase.rem_euclid(1.0);
    let tau = std::f32::consts::TAU;
    match config.shape {
        MotionShape::Ripple => (0.5 + 0.5 * (tau * (x / size - phase)).cos()).clamp(0.0, 1.0),
        MotionShape::Harmonic => (0.5 + 0.5 * (tau * (x / size - phase)).cos())
            .clamp(0.0, 1.0)
            .powi(5),
        MotionShape::Drift => {
            // Integer phase multipliers keep the field continuous when the
            // free-running phase wraps, while the unrelated spatial scales
            // prevent the layers from looking like a simple repeating comb.
            let broad = (tau * (x / (size * 2.7) - phase)).sin();
            let middle = (tau * (x / (size * 1.13) + phase * 2.0 + 0.19)).sin();
            let detail = (tau * (x / (size * 0.53) - phase * 3.0 + 0.61)).sin();
            (0.5 + 0.25 * broad + 0.17 * middle + 0.08 * detail).clamp(0.0, 1.0)
        }
        MotionShape::Scan | MotionShape::Notch => {
            let center = phase * DISPLAY_OCTAVES;
            let distance = (x - center).abs();
            let wrapped = distance.min(DISPLAY_OCTAVES - distance);
            let window = (-0.5 * (wrapped / (size * 0.42)).powi(2)).exp();
            if config.shape == MotionShape::Scan {
                window
            } else {
                1.0 - window
            }
        }
        MotionShape::Saw => {
            let cycle = (x / size - phase).rem_euclid(1.0);
            const RESET_START: f32 = 0.9;
            if cycle < RESET_START {
                cycle / RESET_START
            } else {
                1.0 - smoothstep((cycle - RESET_START) / (1.0 - RESET_START))
            }
        }
        // Sprinkle is event-driven and is applied separately from the looping
        // motion field. Its resting state must therefore be neutral.
        MotionShape::Sprinkle | MotionShape::Cloud => 1.0,
    }
}

pub fn motion_attenuation_db(frequency: f32, config: MotionConfig) -> f32 {
    config.depth_db.max(0.0) * (1.0 - motion_openness(frequency, config))
}

/// Broadband normalization for the moving spectral field. Sampling uniformly
/// in octaves makes compensation follow the shape's perceptual footprint
/// rather than over-weighting its highest-frequency bins. The cap prevents
/// very narrow, deep Scan or Harmonic settings from producing unsafe boosts.
pub fn motion_compensation_gain(config: MotionConfig) -> f32 {
    if config.depth_db <= 0.001 {
        return 1.0;
    }
    let mean_square = (0..MOTION_COMPENSATION_SAMPLES)
        .map(|index| {
            let octave =
                (index as f32 + 0.5) / MOTION_COMPENSATION_SAMPLES as f32 * DISPLAY_OCTAVES;
            let frequency = MIN_DISPLAY_FREQUENCY_HZ * 2.0_f32.powf(octave);
            let gain = db_to_gain(-motion_attenuation_db(frequency, config));
            gain * gain
        })
        .sum::<f32>()
        / MOTION_COMPENSATION_SAMPLES as f32;
    mean_square
        .sqrt()
        .recip()
        .min(db_to_gain(MAX_MOTION_COMPENSATION_DB))
}

fn closest_bin_in_cents(center: f32, bin_hz: f32, max_bin: usize) -> usize {
    let position = center / bin_hz;
    let lower = (position.floor() as usize).clamp(1, max_bin);
    let upper = (lower + 1).min(max_bin);
    let cents = |bin: usize| (1200.0 * (bin as f32 * bin_hz / center).log2()).abs();
    if cents(upper) < cents(lower) {
        upper
    } else {
        lower
    }
}

pub fn manual_gain_at_bin(mask_db: &[f32; MANUAL_MASK_POINTS], bin: usize, fft_size: usize) -> f32 {
    let source_bins = fft_size / 2;
    let master_bins = MANUAL_MASK_POINTS - 1;
    let master_index = (bin * master_bins + source_bins / 2)
        .checked_div(source_bins)
        .unwrap_or(0)
        .min(master_bins);
    let db = mask_db[master_index];
    if db <= MANUAL_CURVE_MUTE_DB {
        0.0
    } else {
        db_to_gain(db.min(0.0))
    }
}

#[inline]
fn smoothing_alpha(elapsed_seconds: f32, time_seconds: f32) -> f32 {
    if time_seconds <= 0.0 {
        1.0
    } else {
        1.0 - (-elapsed_seconds / time_seconds).exp()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
