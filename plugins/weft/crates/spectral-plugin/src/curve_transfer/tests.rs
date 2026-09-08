use super::*;
#[test]
fn transfer_preserves_hidden_values_and_remaps_frequency() {
    let source = std::array::from_fn(|i| i as f32 / MANUAL_MASK_POINTS as f32 * 100.0 - 80.0);
    let text = encode(&source, 48_000.0);
    assert_eq!(decode(&text, 48_000.0).unwrap(), source);
    let at_double_rate = decode(&text, 96_000.0).unwrap();
    assert_eq!(at_double_rate[100], source[200]);
    assert!(decode("not a curve", 48_000.0).is_err());
    assert!(decode(&text.replace("-80\n", "NaN\n"), 48_000.0).is_err());
    assert!(decode(&format!("{text} 1"), 48_000.0).is_err());
}
