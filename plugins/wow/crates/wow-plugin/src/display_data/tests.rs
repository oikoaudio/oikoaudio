use super::*;
#[test]
fn display_history_is_ordered_and_bounded() {
    let display = ModulationDisplay::default();
    for index in 0..DISPLAY_POINTS + 7 {
        display.push(index as f32, -(index as f32));
    }
    let (left, right) = display.snapshot();
    assert_eq!(left.len(), DISPLAY_POINTS);
    assert_eq!(left[0], 7.0);
    assert_eq!(left[DISPLAY_POINTS - 1], (DISPLAY_POINTS + 6) as f32);
    assert_eq!(right[0], -7.0);
}

#[test]
fn clearing_display_hides_old_samples() {
    let display = ModulationDisplay::default();
    display.push(0.1, -0.1);
    display.clear();
    let (left, right) = display.snapshot();
    assert!(left.is_empty());
    assert!(right.is_empty());
}
