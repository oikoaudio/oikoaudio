//! Numerical spectral processing, independent of NicePlug and the editor.
use oiko_dsp::db_to_gain;
const SMOOTH_MASK_TIME_SECONDS: f32 = 0.03;

/// Build the canonical dual window for the given overlap.
/// Panics on mismatched buffers, zero overlap, or a non-divisible window size.
pub fn build_dual_synthesis_window(analysis: &[f32], synthesis: &mut [f32], overlap: usize) {
    assert_eq!(analysis.len(), synthesis.len());
    assert!(overlap > 0 && !analysis.is_empty() && analysis.len().is_multiple_of(overlap));
    let hop = analysis.len() / overlap;
    for (index, output) in synthesis.iter_mut().enumerate() {
        let denominator = (0..overlap)
            .map(|offset| analysis[(index + offset * hop) % analysis.len()].powi(2))
            .sum::<f32>()
            .max(1.0e-12);
        *output = analysis[index] / denominator;
    }
}

/// Smooth adjacent gain bins in decibels, retaining the edge bins. No allocation.
/// Source and output lengths must match.
pub fn soften_spectral_edges(source: &[f32], output: &mut [f32]) {
    debug_assert_eq!(source.len(), output.len());
    if source.len() < 3 {
        output.copy_from_slice(source);
        return;
    }
    output[0] = source[0];
    for index in 1..source.len() - 1 {
        let left_db = 20.0 * source[index - 1].max(1.0e-6).log10();
        let center_db = 20.0 * source[index].max(1.0e-6).log10();
        let right_db = 20.0 * source[index + 1].max(1.0e-6).log10();
        output[index] = db_to_gain(left_db * 0.2 + center_db * 0.6 + right_db * 0.2);
    }
    let last = source.len() - 1;
    output[last] = source[last];
}

/// Approach a target gain mask with a 30 ms time constant. No allocation.
/// Both buffers must have equal lengths and contain finite, nonnegative gains.
pub fn smooth_mask_in_db(current: &mut [f32], target: &[f32], elapsed_seconds: f32) {
    debug_assert_eq!(current.len(), target.len());
    let blend = (1.0 - (-elapsed_seconds / SMOOTH_MASK_TIME_SECONDS).exp()).clamp(0.0, 1.0);
    for (current, target) in current.iter_mut().zip(target) {
        let current_db = 20.0 * current.max(1.0e-6).log10();
        let target_db = 20.0 * target.max(1.0e-6).log10();
        *current = db_to_gain(current_db + (target_db - current_db) * blend);
    }
}

/// Prepared analysis/synthesis windows and frequency lookup for one FFT size.
/// Construct during activation. Processing only borrows these immutable tables.
pub struct PreparedSpectrum {
    pub window: Vec<f32>,
    pub synthesis_window: Vec<f32>,
    pub window_coherent_gain: f32,
    pub smooth_analysis_window: Vec<f32>,
    pub smooth_synthesis_window: Vec<f32>,
    pub smooth_window_coherent_gain: f32,
    pub mask_workspace: crate::MaskWorkspace,
}
impl PreparedSpectrum {
    /// Both input windows must have the same nonzero length divisible by overlap.
    pub fn new(
        sample_rate: f32,
        window: Vec<f32>,
        smooth_analysis_window: Vec<f32>,
        overlap: usize,
    ) -> Self {
        let size = window.len();
        assert_eq!(size, smooth_analysis_window.len());
        let mut synthesis_window = vec![0.0; size];
        let mut smooth_synthesis_window = vec![0.0; size];
        build_dual_synthesis_window(&window, &mut synthesis_window, overlap);
        build_dual_synthesis_window(
            &smooth_analysis_window,
            &mut smooth_synthesis_window,
            overlap,
        );
        let window_coherent_gain = window.iter().sum::<f32>() / size as f32;
        let smooth_window_coherent_gain = smooth_analysis_window.iter().sum::<f32>() / size as f32;
        let mut mask_workspace = crate::MaskWorkspace::default();
        mask_workspace.prepare(sample_rate, size);
        Self {
            window,
            synthesis_window,
            window_coherent_gain,
            smooth_analysis_window,
            smooth_synthesis_window,
            smooth_window_coherent_gain,
            mask_workspace,
        }
    }
}
