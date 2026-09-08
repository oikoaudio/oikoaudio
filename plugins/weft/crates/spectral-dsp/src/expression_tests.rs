use super::*;

fn masks(gain: f32, pan: f32) -> (Vec<f32>, Vec<f32>, MaskConfig) {
    let config = MaskConfig {
        fft_size: 4096,
        note_depth_db: 24.0,
        width_cents: 100.0,
        ..Default::default()
    };
    let mut workspace = MaskWorkspace::default();
    workspace.prepare(config.sample_rate, config.fft_size);
    let mut left = vec![0.0; 2049];
    let mut right = left.clone();
    build_stereo_masks_with_voices_precomputed(
        &mut left,
        &mut right,
        &[0.0; MANUAL_MASK_POINTS],
        &[MaskVoice {
            note: 69,
            level: 1.0,
            expression_gain: gain,
            pan,
            ..Default::default()
        }],
        config,
        &workspace,
    );
    (left, right, config)
}
#[test]
fn gain_scales_the_note_component_above_unity_without_changing_the_floor() {
    let (unity, _, config) = masks(1.0, 0.0);
    let (louder, _, _) = masks(2.0, 0.0);
    let (muted, _, _) = masks(0.0, 0.0);
    let floor = db_to_gain(-config.note_depth_db);
    for ((a, b), zero) in unity.iter().zip(&louder).zip(&muted) {
        assert!((b - floor - 2.0 * (a - floor)).abs() < 1e-6);
        assert!((zero - floor).abs() < 1e-6);
    }
    assert!(louder.iter().copied().fold(0.0, f32::max) > 2.0);
}
#[test]
fn pan_is_symmetric_and_center_preserves_the_mono_mask() {
    let (center_l, center_r, config) = masks(1.0, 0.0);
    assert_eq!(center_l, center_r);
    let mut mono = vec![0.0; center_l.len()];
    build_mask_with_voices(
        &mut mono,
        &[0.0; MANUAL_MASK_POINTS],
        &[MaskVoice {
            note: 69,
            level: 1.0,
            ..Default::default()
        }],
        config,
    );
    assert!(
        mono.iter()
            .zip(&center_l)
            .all(|(a, b)| (a - b).abs() < 0.002)
    );
    let (left_l, left_r, _) = masks(1.0, -1.0);
    let (right_l, right_r, _) = masks(1.0, 1.0);
    let floor = db_to_gain(-config.note_depth_db);
    assert_eq!(left_l, right_r);
    assert_eq!(left_r, right_l);
    assert!(left_r.iter().all(|gain| (*gain - floor).abs() < 1e-6));
    for (left, center) in left_l.iter().zip(&center_l) {
        assert!((left - floor - (center - floor) * std::f32::consts::SQRT_2).abs() < 1e-6);
    }
}
#[test]
fn overlapping_same_pitch_voices_can_open_opposite_sides_independently() {
    let (expected_l, _, config) = masks(1.0, -1.0);
    let (_, expected_r, _) = masks(0.5, 1.0);
    let mut workspace = MaskWorkspace::default();
    workspace.prepare(config.sample_rate, config.fft_size);
    let mut left = vec![0.0; expected_l.len()];
    let mut right = left.clone();
    let voices = [
        MaskVoice {
            note: 69,
            level: 1.0,
            pan: -1.0,
            ..Default::default()
        },
        MaskVoice {
            note: 69,
            level: 1.0,
            pan: 1.0,
            expression_gain: 0.5,
            ..Default::default()
        },
    ];
    build_stereo_masks_with_voices_precomputed(
        &mut left,
        &mut right,
        &[0.0; MANUAL_MASK_POINTS],
        &voices,
        config,
        &workspace,
    );
    assert_eq!(left, expected_l);
    assert_eq!(right, expected_r);
}
