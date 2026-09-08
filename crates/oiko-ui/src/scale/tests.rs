use super::*;
#[test]
fn malformed_and_out_of_range_preferences_are_safe() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(nearest_scale(value), 1.0);
    }
    assert_eq!(nearest_scale(-9.0), 0.5);
    assert_eq!(nearest_scale(100.0), 2.0);
    assert_eq!(nearest_scale(1.13), 1.25);
    for step in SCALE_STEPS {
        assert_eq!(nearest_scale(step), step);
    }
}
