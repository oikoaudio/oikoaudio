use inton_core::{
    scale_edit::Draft,
    tuning::{Preset, twelve_edo},
};
#[test]
fn unedited_copy_preserves_ratios_and_mapping_exactly() {
    let mut p = twelve_edo();
    p.kbm_text = Some("0\n0\n127\n60\n69\n442.0\n12\n".into());
    let draft = Draft::new(p.clone()).unwrap();
    assert_eq!(draft.preset().unwrap(), p);
}
#[test]
fn edits_are_isolated_validated_and_embedded() {
    let p = twelve_edo();
    let mut draft = Draft::new(p.clone()).unwrap();
    draft.degrees[3] = 386.3137138648;
    let edited = draft.preset().unwrap();
    let t = edited.prepare().unwrap();
    assert!((t.hz[64] / t.hz[60] - 1.25).abs() < 1e-10);
    assert_eq!(p, twelve_edo());
    let mut project = inton_core::state::Project::default();
    project.assign(3, edited).unwrap();
    let restored = inton_core::state::Project::decode(&project.encode().unwrap()).unwrap();
    assert_eq!(restored.slots[3], project.slots[3]);
    draft.degrees[3] = f64::NAN;
    assert!(draft.preset().is_err());
}
#[test]
fn generators_support_non_octave_periods_and_safe_mapping_reset() {
    let mut draft = Draft::new(twelve_edo()).unwrap();
    draft.equal_divisions(13, 1200.0 * 3.0_f64.log2()).unwrap();
    let t = draft.preset().unwrap().prepare().unwrap();
    assert_eq!(t.count, 13);
    assert!((t.hz[73] / t.hz[60] - 3.0).abs() < 1e-10);
    assert!(draft.equal_divisions(4097, 1200.0).is_err());
    assert!(draft.equal_divisions(0, 1200.0).is_err());
    assert!(draft.equal_divisions(12, f64::INFINITY).is_err());
}
#[test]
fn large_tables_can_be_edited_and_extract_only_an_explicit_cycle() {
    let p = Preset::new(
        "Table".into(),
        format!(
            "Table\n128\n{}",
            (1..=128)
                .map(|i| format!("{:.6}\n", i as f64 * 1200.0 / 7.0))
                .collect::<String>()
        ),
        None,
    );
    let mut draft = Draft::new(p).unwrap();
    draft.degrees[126] += 10.;
    assert_eq!(draft.preset().unwrap().prepare().unwrap().count, 128);
    draft.extract_cycle(14, 7).unwrap();
    assert_eq!(draft.degrees.len(), 7);
    assert!((draft.degrees[6] - 1200.).abs() < 1e-6);
    assert_eq!(draft.root, 60);
    assert_eq!(draft.reference, 69);
    assert!(draft.extract_cycle(12, 7).is_err());
}

#[test]
fn note_edits_preserve_period_and_other_intervals() {
    let mut draft = Draft::new(inton_core::tuning::twelve_edo()).unwrap();
    draft.equal_divisions(7, 1200.).unwrap();
    let before = draft.degrees.clone();
    draft.add_note().unwrap();
    assert_eq!(draft.degrees.len(), 8);
    assert_eq!(&draft.degrees[..6], &before[..6]);
    assert_eq!(*draft.degrees.last().unwrap(), 1200.);
    draft.remove_note(6).unwrap();
    assert_eq!(draft.degrees, before);
    assert!(draft.remove_note(6).is_err()); // Period is not a removable note.
    draft.equal_divisions(1, 3600.).unwrap();
    assert!(draft.remove_note(0).is_err());
    draft.add_note().unwrap();
    assert_eq!(draft.degrees, vec![1800., 3600.]);
    assert_eq!(draft.offset_baseline(0), 1800.);
    assert!(draft.preset().unwrap().prepare().is_ok());
}

#[test]
fn single_scale_default_and_set_intent_survive_recall() {
    let mut project = inton_core::state::Project::default();
    assert!(!project.uses_scale_set());
    project.scale_set = true;
    assert!(
        inton_core::state::Project::decode(&project.encode().unwrap())
            .unwrap()
            .uses_scale_set()
    );
    project.scale_set = false;
    project.assign(2, inton_core::tuning::twelve_edo()).unwrap();
    project.clear(2).unwrap();
    assert!(project.uses_scale_set());
}

#[test]
fn insert_in_the_middle_preserves_existing_pitches_and_can_be_removed() {
    let preset = inton::library::Library::factory()
        .load("factory:7edo")
        .unwrap();
    let mut draft = inton_core::scale_edit::Draft::new(preset).unwrap();
    let before = draft.degrees.clone();
    draft.insert_note(2).unwrap();
    assert_eq!(draft.degrees.len(), 8);
    assert_eq!(draft.degrees[2], (before[1] + before[2]) / 2.);
    assert_eq!(&draft.degrees[3..], &before[2..]);
    draft.remove_note(2).unwrap();
    assert_eq!(draft.degrees, before);
    draft.insert_note(0).unwrap();
    assert_eq!(draft.degrees[0], before[0] / 2.);
}
