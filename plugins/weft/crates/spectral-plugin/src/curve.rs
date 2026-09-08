//! Curve transform calculations shared by processing and editor previews.
use spectral_dsp::{MANUAL_CURVE_MUTE_DB, MANUAL_MASK_POINTS};

pub(crate) const CURVE_TRANSFORM_STORAGE_LIMIT_DB: f32 = 240.0;

pub(crate) fn transformed_curve_db(
    curve: &[f32; MANUAL_MASK_POINTS],
    output_index: usize,
    sample_rate: f32,
    depth_percent: f32,
    tilt_db_per_octave: f32,
    shift_semitones: f32,
) -> f32 {
    let shift_ratio = 2.0_f32.powf(shift_semitones / 12.0);
    let source_position =
        (output_index as f32 / shift_ratio).clamp(0.0, (MANUAL_MASK_POINTS - 1) as f32);
    let left = source_position.floor() as usize;
    let right = (left + 1).min(MANUAL_MASK_POINTS - 1);
    let fraction = source_position - left as f32;
    let shifted_db = curve[left] + (curve[right] - curve[left]) * fraction;
    let frequency =
        (output_index as f32 / (MANUAL_MASK_POINTS - 1) as f32 * sample_rate.max(1.0) * 0.5)
            .max(20.0);
    let transformed_db = shifted_db + tilt_db_per_octave * (frequency / 1000.0).log2();
    (transformed_db * (depth_percent * 0.01).clamp(0.0, 8.0)).clamp(
        -CURVE_TRANSFORM_STORAGE_LIMIT_DB,
        CURVE_TRANSFORM_STORAGE_LIMIT_DB,
    )
}

pub(crate) fn transform_curve(
    source: &[f32; MANUAL_MASK_POINTS],
    output: &mut [f32; MANUAL_MASK_POINTS],
    tilt_octaves: &[f32; MANUAL_MASK_POINTS],
    depth_percent: f32,
    tilt_db_per_octave: f32,
    shift_semitones: f32,
) {
    let shift_ratio = 2.0_f32.powf(shift_semitones / 12.0);
    let depth = (depth_percent * 0.01).clamp(0.0, 8.0);
    for (index, value) in output.iter_mut().enumerate() {
        let source_position =
            (index as f32 / shift_ratio).clamp(0.0, (MANUAL_MASK_POINTS - 1) as f32);
        let left = source_position.floor() as usize;
        let right = (left + 1).min(MANUAL_MASK_POINTS - 1);
        let fraction = source_position - left as f32;
        let shifted_db = source[left] + (source[right] - source[left]) * fraction;
        *value = ((shifted_db + tilt_db_per_octave * tilt_octaves[index]).min(0.0) * depth)
            .clamp(MANUAL_CURVE_MUTE_DB, 0.0);
    }
}
