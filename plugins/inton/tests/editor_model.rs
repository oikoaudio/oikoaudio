use inton::editor_model::{ScaleShape, SetView, ViewPreferences};
use inton_core::{state::Project, tuning::Preset};

#[test]
fn compact_set_preserves_sparse_slots_without_revealing_automation() {
    let mut project = Project::default();
    let mut view = SetView::default();
    assert_eq!(view.rows(&project, 0), vec![0]);
    project
        .assign(11, inton_core::tuning::twelve_edo())
        .unwrap();
    assert_eq!(view.rows(&project, 31), vec![0, 11]);
    assert_eq!(view.add_slot(&project), Some(1));
    assert_eq!(view.destination, 1);
    assert_eq!(view.rows(&project, 0), vec![0, 1, 11]);
    view.destination = 7;
    assert!(!view.rows(&project, 0).contains(&7));
    assert_eq!(project.slots[11].as_ref().unwrap().display_name, "12 EDO");
}

#[test]
fn preferences_with_missing_view_fields_load_and_round_trip() {
    let old = r#"{"folder":"/tmp/scales","favorites":[]}"#;
    let mut prefs: inton::library::Preferences = serde_json::from_str(old).unwrap();
    assert!(!prefs.view.browser_open);
    assert!(prefs.view.dark);
    assert_eq!(prefs.view.scale, 1.0);
    prefs.view.browser_open = false;
    prefs.view.dark = false;
    prefs.view.scale = 1.5;
    let saved = serde_json::to_string(&prefs).unwrap();
    let restored: inton::library::Preferences = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored.view, prefs.view);
    assert_eq!(ViewPreferences::nearest_scale(1.4), 1.5);
    assert_eq!(ViewPreferences::nearest_scale(f64::NAN), 1.0);
}

#[test]
fn scale_geometry_uses_all_degrees_and_the_actual_period() {
    let lib = inton::library::Library::factory();
    for (id, n, period) in [
        ("factory:12edo", 12, 2.0),
        ("factory:13ed3", 13, 3.0),
        ("factory:13ed8", 13, 8.0),
    ] {
        let p = lib.load(id).unwrap();
        let shape = ScaleShape::from_prepared(&p.prepare().unwrap());
        assert_eq!(shape.positions.len(), n);
        assert!(shape.equal_division);
        assert!((shape.period_ratio - period).abs() < 1e-6);
        for (i, x) in shape.positions.iter().enumerate() {
            assert!((x - i as f64 / n as f64).abs() < 1e-6);
        }
    }
    let p = Preset::new(
        "Unequal".into(),
        "Unequal\n4\n9/8\n5/4\n3/2\n2/1\n".into(),
        None,
    );
    let shape = ScaleShape::from_prepared(&p.prepare().unwrap());
    assert!(!shape.equal_division);
    assert!((shape.positions[1] - (9.0_f64 / 8.0).log2()).abs() < 1e-6);
    let scl = format!(
        "Dense\n256\n{}",
        (1..=256)
            .map(|i| format!("{:.8}\n", i as f64 * 1200.0 / 256.0))
            .collect::<String>()
    );
    let shape =
        ScaleShape::from_prepared(&Preset::new("Dense".into(), scl, None).prepare().unwrap());
    assert_eq!(shape.positions.len(), 256);
    assert!(shape.equal_division);
}

#[test]
fn cleared_slots_remain_visible_and_portable_without_allocating_host_positions() {
    let mut project = Project::default();
    project.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    project.assign(2, inton_core::tuning::twelve_edo()).unwrap();
    project.clear(1).unwrap();
    let recalled = Project::decode(&project.encode().unwrap()).unwrap();
    let view = SetView::default();
    assert_eq!(view.rows(&recalled, 31), vec![0, 1, 2]);
    assert!(recalled.slots[1].is_none());
    let mut legacy = serde_json::to_value(Project::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("allocated_slots");
    let legacy = Project::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(view.rows(&legacy, 31), vec![0]);
}

#[test]
fn x_removes_only_the_target_when_no_occupied_slot_follows() {
    let mut project = Project::default();
    for n in 1..4 {
        project.assign(n, inton_core::tuning::twelve_edo()).unwrap();
    }
    project.clear(1).unwrap();
    assert!(project.has_slot(1));
    project.clear(2).unwrap();
    assert!(project.has_slot(2));
    project.clear(3).unwrap();
    assert!(!project.has_slot(3));
    // Other empty rows aren't silently pruned. Each × removes its own row.
    assert!(project.has_slot(1) && project.has_slot(2));
    project.clear(1).unwrap();
    assert!(!project.has_slot(1));
    let project = Project::decode(&project.encode().unwrap()).unwrap();
    assert_eq!(SetView::default().rows(&project, 31), vec![0, 2]);
}

#[test]
fn set_navigation_skips_gaps_and_stops_at_ends() {
    let mut project = Project::default();
    project.assign(3, inton_core::tuning::twelve_edo()).unwrap();
    project.assign(7, inton_core::tuning::twelve_edo()).unwrap();
    use inton::editor_model::adjacent_slot;
    assert_eq!(adjacent_slot(&project, 0, false), None);
    assert_eq!(adjacent_slot(&project, 0, true), Some(3));
    assert_eq!(adjacent_slot(&project, 3, true), Some(7));
    assert_eq!(adjacent_slot(&project, 7, true), None);
    assert_eq!(adjacent_slot(&project, 7, false), Some(3));
    assert_eq!(adjacent_slot(&project, 2, false), Some(0));
    assert_eq!(adjacent_slot(&project, 2, true), Some(3));
}
