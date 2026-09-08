use super::*;

#[test]
fn offline_render_is_finite_and_non_silent() {
    let input = dense_test_signal(0.5);
    let mut notes = [0.0; MIDI_NOTES];
    notes[69] = 1.0;
    let output = process_offline(&input, &notes);
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(output.iter().any(|sample| sample.abs() > 1.0e-5));
}
