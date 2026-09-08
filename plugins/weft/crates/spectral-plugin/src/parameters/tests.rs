use super::*;

#[test]
fn synchronized_motion_divisions_follow_quarter_note_tempo() {
    assert!((MotionRateDivision::Quarter.rate_hz(120.0) - 2.0).abs() < 1.0e-6);
    assert_eq!(MotionRateDivision::ThreeWholeNotes.beats(), 12.0);
    assert_eq!(MotionRateDivision::WholeDotted.beats(), 6.0);
    assert!((MotionRateDivision::ThreeWholeNotes.rate_hz(120.0) - 1.0 / 6.0).abs() < 1.0e-6);
    assert!((MotionRateDivision::WholeDotted.rate_hz(120.0) - 1.0 / 3.0).abs() < 1.0e-6);
    assert_eq!(
        MotionRateDivision::closest_to_hz(1.0 / 6.0, 120.0),
        MotionRateDivision::ThreeWholeNotes
    );
    assert_eq!(
        MotionRateDivision::closest_to_hz(1.0 / 3.0, 120.0),
        MotionRateDivision::WholeDotted
    );
    assert!((MotionRateDivision::Eighth.rate_hz(120.0) - 4.0).abs() < 1.0e-6);
    assert!((MotionRateDivision::QuarterTriplet.beats() - 2.0 / 3.0).abs() < 1.0e-6);
    assert_eq!(
        MotionRateDivision::SixtyFourth.rate_hz(120.0),
        MAX_FREE_MOTION_RATE_HZ
    );
    assert!(
        MotionRateDivision::ALL
            .windows(2)
            .all(|pair| pair[0].beats() > pair[1].beats())
    );
}

#[test]
fn switching_to_sync_selects_the_nearest_equivalent_division() {
    assert_eq!(
        MotionRateDivision::closest_to_hz(0.125, 120.0),
        MotionRateDivision::FourWholeNotes
    );
    assert_eq!(
        MotionRateDivision::closest_to_hz(2.0, 120.0),
        MotionRateDivision::Quarter
    );
}
