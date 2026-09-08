use super::*;

#[test]
fn capture_is_time_weighted_and_independent_of_editor_reads() {
    let display = CaptureDisplay::default();
    let mut capture = CaptureAccumulator::default();
    let epoch = display.begin();
    capture.push(&[0.0; ANALYZER_POINTS], 0.01, &display);
    capture.push(&[-20.0; ANALYZER_POINTS], 0.03, &display);
    let (power, seconds) = display.read(epoch).unwrap();
    assert!((power[0] - 0.2575).abs() < 1e-6);
    assert!((seconds - 0.04).abs() < 1e-6);
    display.stop();
    capture.push(&[6.0; ANALYZER_POINTS], 1.0, &display);
    assert_eq!(display.read(epoch).unwrap().0, power);
    let next = display.begin();
    assert!(display.read(next).is_none());
    capture.push(&[-20.0; ANALYZER_POINTS], 0.01, &display);
    assert!((display.read(next).unwrap().0[0] - 0.01).abs() < 1e-6);
}
