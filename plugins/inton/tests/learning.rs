use inton::library::Library;
#[test]
fn guided_scales_load_and_have_listening_hints() {
    let lib = Library::factory();
    let mut lessons: Vec<_> = lib
        .entries
        .iter()
        .filter_map(|e| {
            e.tags
                .iter()
                .find_map(|t| t.strip_prefix("lesson:").map(|rank| (rank, e)))
        })
        .collect();
    lessons.sort_by_key(|(rank, _)| *rank);
    assert!(!lessons.is_empty());
    for (_, e) in lessons {
        let preset = lib.load(&e.id).unwrap();
        assert!(!inton::listening::hint(&preset).is_empty());
        assert!(lib.load(&e.id).unwrap().prepare().is_ok());
    }
}
#[test]
fn carlos_equal_steps_have_the_documented_non_octave_spans() {
    let lib = Library::factory();
    for (id, count, step) in [("alpha", 9, 78.0), ("beta", 11, 63.8), ("gamma", 20, 35.1)] {
        let p = lib.load(&format!("factory:carlos-{id}")).unwrap();
        let t = p.prepare().unwrap();
        assert_eq!(t.count, count);
        assert!(p.source.contains("wendycarlos.com"));
        for n in 0..127 {
            assert!((1200. * (t.hz[n + 1] / t.hz[n]).log2() - step).abs() < 1e-7);
        }
        assert!((1200. * t.period.log2() - step * count as f64).abs() < 1e-7);
        assert!((t.period - 2.).abs() > 0.1);
    }
}

#[test]
fn exercises_match_the_actual_scale_and_do_not_change_project_data() {
    let mut preset = Library::factory().load("factory:19edo").unwrap();
    let description = preset.description.clone();
    preset.description = "Cached factory description".into();
    assert_eq!(inton::listening::description(&preset), description);
    let before = serde_json::to_string(&preset).unwrap();
    assert!(!inton::listening::hint(&preset).is_empty());
    assert_eq!(serde_json::to_string(&preset).unwrap(), before);
    preset.scl_text = inton_core::tuning::twelve_edo().scl_text;
    assert_eq!(
        inton::listening::description(&preset),
        "Cached factory description"
    );
    assert!(
        inton::listening::hint(&preset).is_empty(),
        "Edited scales must not inherit inaccurate exercises"
    );
    assert!(
        !inton::listening::hint(&inton_core::tuning::twelve_edo()).is_empty(),
        "The initial default scale should have a listening idea"
    );
}
#[test]
fn listening_ideas_are_an_opt_in_global_preference() {
    let old = r#"{"folder":"/tmp/scales","favorites":[],"view":{"dark":false,"scale":1.25}}"#;
    let mut prefs: inton::library::Preferences = serde_json::from_str(old).unwrap();
    assert!(!prefs.view.listening_ideas);
    prefs.view.listening_ideas = true;
    let path =
        std::env::temp_dir().join(format!("inton-listening-prefs-{}.json", std::process::id()));
    prefs.save_to(&path).unwrap();
    let restored = inton::library::Preferences::load_from(&path).unwrap();
    assert!(restored.view.listening_ideas);
    assert!(!restored.view.dark);
    assert_eq!(restored.view.scale, 1.25);
    std::fs::remove_file(path).unwrap();
    assert!(
        !String::from_utf8(inton_core::state::Project::default().encode().unwrap())
            .unwrap()
            .contains("listening_ideas")
    );
}

#[test]
fn all_factory_exercises_have_separate_metadata() {
    let records: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../resources/library.json")).unwrap();
    let lib = Library::factory();
    for record in records {
        let id = record["source_id"].as_str().unwrap();
        let hint = record["listening_hint"]
            .as_str()
            .expect("Every factory entry declares its optional listening hint");
        let preset = lib.load(id).unwrap();
        assert_eq!(inton::listening::hint(&preset), hint, "{id}");
        if id.starts_with("factory:detune-") {
            assert!(!hint.is_empty(), "Missing electronic listening idea: {id}");
        }
    }
}

#[test]
fn imported_description_does_not_repeat_the_title_or_modify_the_file() {
    let mut p = inton_core::tuning::Preset::new(
        "Seven-note cycle".into(),
        "Seven-note cycle\n7\n200.0\n400.0\n500.0\n700.0\n900.0\n1100.0\n1200.0\n".into(),
        None,
    );
    p.description = " Seven-note cycle ".into();
    let before = serde_json::to_string(&p).unwrap();
    assert_eq!(inton::listening::description(&p), "");
    assert_eq!(serde_json::to_string(&p).unwrap(), before);
    p.description = "Additional information about this tuning.".into();
    assert_eq!(inton::listening::description(&p), p.description);
}

#[test]
fn seven_note_import_gets_mapping_aware_optional_guidance() {
    let mut p = inton_core::tuning::Preset::new(
        "Cycle".into(),
        "Cycle\n7\n200.0\n400.0\n500.0\n700.0\n900.0\n1100.0\n1200.0\n".into(),
        None,
    );
    let t = p.prepare().unwrap();
    let hint = inton::listening::hint_for(&p, &t);
    assert!(!hint.is_empty());
    p.kbm_text = Some("0\n0\n127\n60\n69\n440.0\n0\n".into());
    assert!(inton::listening::hint_for(&p, &p.prepare().unwrap()).is_empty());
}
