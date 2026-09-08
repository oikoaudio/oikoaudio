use super::*;
#[test]
fn amplitude_and_pitch_reference_values() {
    assert_eq!(db_to_gain(0.0), 1.0);
    assert!((db_to_gain(-20.0) - 0.1).abs() < 1e-7);
    assert_eq!(note_frequency(69), 440.0);
    assert_eq!(note_frequency(57), 220.0);
    assert!((note_frequency_with_tuning(69, 1.0) - 466.16376).abs() < 1e-3);
}
