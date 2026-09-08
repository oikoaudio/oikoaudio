use inton_core::{
    runtime::Shared,
    scale_edit::Draft,
    state::ValidatedProject,
    tuning::{ValidatedScale, twelve_edo},
};
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn a_draft_revision_is_shared_by_audition_apply_and_history() {
    let original = ValidatedScale::new(twelve_edo()).unwrap();
    let shared = Shared::new();
    shared.assign_prepared(0, original.clone()).unwrap();
    let saved = shared.snapshot().encode().unwrap();
    let mut draft = Draft::from_validated(original.clone());
    assert!(std::ptr::eq(
        draft.validated().unwrap().tuning(),
        original.tuning()
    ));

    draft.degrees[0] += 20.0;
    let edited = draft.validated().unwrap();
    assert!(std::ptr::eq(
        edited.tuning(),
        draft.validated().unwrap().tuning()
    ));
    assert_ne!(edited.tuning().hz[61], original.tuning().hz[61]);
    shared.audition_enabled.store(true, SeqCst);
    shared.audition_prepared(edited.clone());
    assert_eq!(shared.publication(0.0).0, edited.tuning().hz);
    assert_eq!(shared.snapshot().encode().unwrap(), saved);
    shared.stop_audition();
    assert!((shared.publication(0.0).0[61] - original.tuning().hz[61]).abs() < 1e-10);

    shared
        .assign_prepared(0, draft.validated().unwrap())
        .unwrap();
    assert!(std::ptr::eq(
        shared.scale(0).unwrap().tuning(),
        edited.tuning()
    ));
    shared.undo_edit(false).unwrap();
    assert!(std::ptr::eq(
        shared.scale(0).unwrap().tuning(),
        original.tuning()
    ));
    shared.undo_edit(true).unwrap();
    assert!(std::ptr::eq(
        shared.scale(0).unwrap().tuning(),
        edited.tuning()
    ));

    draft.degrees[0] += 10.0;
    let next = draft.validated().unwrap();
    assert!(!std::ptr::eq(next.tuning(), edited.tuning()));
    draft.degrees[0] = f64::NAN;
    assert!(draft.validated().is_err());
    assert!(std::ptr::eq(
        shared.scale(0).unwrap().tuning(),
        edited.tuning()
    ));
}

#[test]
fn slot_edits_retain_other_prepared_scales_and_recall_keeps_the_wire_format() {
    let shared = Shared::new();
    let first = shared.scale(0).unwrap();
    let second = ValidatedScale::new(twelve_edo()).unwrap();
    let slot = shared.append_prepared(second.clone()).unwrap();
    assert!(std::ptr::eq(
        shared.scale(0).unwrap().tuning(),
        first.tuning()
    ));
    assert!(std::ptr::eq(
        shared.scale(slot).unwrap().tuning(),
        second.tuning()
    ));
    shared.clear(slot).unwrap();
    assert!(std::ptr::eq(
        shared.scale(0).unwrap().tuning(),
        first.tuning()
    ));
    shared.undo_edit(false).unwrap();
    assert!(std::ptr::eq(
        shared.scale(slot).unwrap().tuning(),
        second.tuning()
    ));
    let bytes = shared.snapshot().encode().unwrap();
    shared
        .restore_prepared(ValidatedProject::decode(&bytes).unwrap())
        .unwrap();
    assert_eq!(shared.snapshot().encode().unwrap(), bytes);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value.get("scales").is_none());
    assert!(value["slots"][0].get("tuning").is_none());
}
