use super::*;

#[test]
fn divisions_follow_tempo_and_stay_in_their_lfo_domain() {
    assert!((RateDivision::SixteenthTriplet.rate_hz(120.0) - 12.0).abs() < 1e-5);
    assert!((RateDivision::Quarter.rate_hz(90.0) - 1.5).abs() < 1e-5);
    for tempo in 1..=960 {
        for range in [WOW, FLUTTER] {
            let (first, last) = range.division_indices(tempo as f32);
            assert!(first <= last);
            for requested in RateDivision::ALL {
                let actual = requested.bounded(tempo as f32, range).rate_hz(tempo as f32);
                assert!(
                    (range.min..=range.max).contains(&actual),
                    "{tempo}: {actual}"
                );
            }
        }
    }
}

#[test]
fn division_order_and_hz_positions_round_trip() {
    let params = crate::WowParams::default();
    for pair in RateDivision::ALL.windows(2) {
        assert!(pair[0].beats() > pair[1].beats());
        assert_eq!(pair[0].to_index() + 1, pair[1].to_index());
    }
    for tempo in [30.0, 90.0, 120.0, 143.0, 300.0] {
        for (range, free) in [(WOW, &params.rate), (FLUTTER, &params.flutter_rate)] {
            let (first, last) = range.division_indices(tempo);
            for division in &RateDivision::ALL[first..=last] {
                let hz = division.rate_hz(tempo);
                let position = free.preview_normalized(hz);
                assert!((free.preview_plain(position) - hz).abs() < 1e-4);
                assert_eq!(
                    RateDivision::closest_to_hz(free.preview_plain(position), tempo, range),
                    *division
                );
            }
            for step in 0..=100 {
                let hz = free.preview_plain(step as f32 / 100.0);
                let selected = RateDivision::closest_to_hz(hz, tempo, range);
                let distance = (selected.rate_hz(tempo) / hz).ln().abs();
                assert!(
                    RateDivision::ALL[first..=last]
                        .iter()
                        .all(|d| (d.rate_hz(tempo) / hz).ln().abs() >= distance)
                );
            }
        }
    }
}

#[test]
fn invalid_or_missing_tempo_retains_last_valid_value() {
    for invalid in [
        None,
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(0.0),
        Some(-1.0),
        Some(f64::MAX),
    ] {
        assert_eq!(host_tempo(invalid, 143.0), 143.0);
    }
    assert_eq!(host_tempo(Some(90.0), 143.0), 90.0);
}
