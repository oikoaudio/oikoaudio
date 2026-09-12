use super::*;

#[test]
fn anchors_follow_song_position_and_only_reset_affected_clocks() {
    let mut clock = PhaseSync::default();
    let divisions = [Some(2.0), Some(0.25)];
    assert_eq!(
        clock.anchors(true, Some(-0.5), 0.01, 10, divisions),
        [Some(0.75), Some(0.0)]
    );
    assert_eq!(
        clock.anchors(true, Some(-0.4), 0.01, 10, divisions),
        [None; 2]
    );
    assert_eq!(
        clock.anchors(true, Some(1.25), 0.01, 10, divisions),
        [Some(0.625), Some(0.0)]
    );
    assert_eq!(
        clock.anchors(true, Some(1.35), 0.01, 10, [Some(1.0), divisions[1]]),
        [Some(1.35_f64.rem_euclid(1.0)), None]
    );
}

#[test]
fn stopped_missing_position_and_unsynced_modes_do_not_anchor() {
    let mut clock = PhaseSync::default();
    assert_eq!(
        clock.anchors(false, Some(1.0), 0.01, 10, [Some(2.0); 2]),
        [None; 2]
    );
    assert_eq!(
        clock.anchors(true, None, 0.01, 10, [Some(2.0); 2]),
        [None; 2]
    );
    assert_eq!(
        clock.anchors(true, Some(f64::NAN), 0.01, 10, [Some(2.0); 2]),
        [None; 2]
    );
    assert_eq!(
        clock.anchors(true, Some(1.0), 0.01, 10, [None, Some(2.0)]),
        [None, Some(0.5)]
    );
}
