use super::*;

#[test]
fn right_edge_selects_the_22khz_display_bin() {
    let plot = Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(750.0, 290.0));
    let fft_size = 16_384;
    let sample_rate = 48_000.0;
    let expected =
        (display_max_frequency(sample_rate) * fft_size as f32 / sample_rate).round() as usize;
    assert_eq!(
        x_to_active_bin(plot.right(), plot, sample_rate, fft_size),
        expected
    );
    assert_eq!(
        clamp_to_rect(plot.right_top() + Vec2::new(8.0, -8.0), plot),
        plot.right_top()
    );
}

#[test]
fn ruler_intro_is_a_single_bounded_left_to_right_wave() {
    assert!(ruler_intro_strength(0.0, 0.15) > ruler_intro_strength(1.0, 0.15));
    assert!(
        ruler_intro_strength(1.0, RULER_INTRO_DURATION_SECONDS - 0.15)
            > ruler_intro_strength(0.0, RULER_INTRO_DURATION_SECONDS - 0.15)
    );
    assert_eq!(ruler_intro_strength(0.5, RULER_INTRO_DURATION_SECONDS), 0.0);
    for position in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let strength = ruler_intro_strength(position, 0.7);
        assert!((0.0..=1.0).contains(&strength));
    }
}
