use super::*;
#[test]
fn persistence_keeps_product_defaults_and_rejects_nonfinite_zoom() {
    assert_eq!(UiScaleState::<100>::default().get(), 1.0);
    let state = UiScaleState::<125>::default();
    assert_eq!(state.get(), 1.25);
    let shared = state.clone();
    PersistentField::set(&state, f32::NAN);
    assert_eq!(shared.get(), 1.0);
    state.set(1.5);
    assert_eq!(PersistentField::map(&shared, |v| *v), 1.5);
}
