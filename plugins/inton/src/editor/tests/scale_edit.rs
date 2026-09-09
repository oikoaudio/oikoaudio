use super::*;

#[test]
fn unified_editor_add_remove_and_cancel_preserve_project() {
    let shared = Arc::new(Shared::new());
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    e.edit_selected().unwrap();
    e.edit
        .as_mut()
        .unwrap()
        .draft
        .equal_divisions(7, 3600.)
        .unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "edit-add-note"));
    assert_eq!(e.edit.as_ref().unwrap().draft.degrees.len(), 8);
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "edit-remove-note:0"));
    assert_eq!(e.edit.as_ref().unwrap().draft.degrees.len(), 7);
    assert_eq!(
        *e.edit.as_ref().unwrap().draft.degrees.last().unwrap(),
        3600.
    );
    click(&mut e, &ctx, &mut time, point(&ctx, "edit-cancel"));
    assert!(e.edit.is_none());
    assert_eq!(shared.snapshot().encode().unwrap(), before);
}

#[test]
fn draft_undo_keys_are_captured_from_the_host() {
    assert_eq!(host_key_capture(false, false), KeyCapture::IgnoreAll);
    assert_eq!(host_key_capture(true, true), KeyCapture::CaptureAll);
    assert_eq!(
        host_key_capture(true, false),
        KeyCapture::CaptureCommands(vec![
            HostKey::Character("z".into()),
            HostKey::Character("Z".into()),
        ])
    );
}

#[test]
fn scale_editor_add_remove_and_undo_update_the_draft() {
    let shared = Arc::new(Shared::new());
    shared
        .assign(0, Library::factory().load("factory:7edo").unwrap())
        .unwrap();
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    e.edit_selected().unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    frame(&mut e, &ctx, &mut time, vec![]);

    click(&mut e, &ctx, &mut time, point(&ctx, "edit-add-note"));
    frame(&mut e, &ctx, &mut time, vec![]);

    assert_eq!(e.edit.as_ref().unwrap().count, 8);
    assert_eq!(e.edit.as_ref().unwrap().draft.degrees.len(), 8);

    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
    );
    assert_eq!(e.edit.as_ref().unwrap().draft.degrees.len(), 7);
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        }],
    );
    assert_eq!(e.edit.as_ref().unwrap().draft.degrees.len(), 8);
    click(
        &mut e,
        &ctx,
        &mut time,
        point(&ctx, "edit-remove-last-note"),
    );
    assert_eq!(e.edit.as_ref().unwrap().draft.degrees.len(), 7);
}

#[test]
fn edit_mode_cancel_is_silent_and_apply_targets_the_inspected_slot() {
    let shared = Arc::new(Shared::new());
    shared.assign(3, inton_core::tuning::twelve_edo()).unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    e.inspect_slot(3);
    let before = shared.snapshot().encode().unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let edit = point(&ctx, "edit-scale");
    click(&mut e, &ctx, &mut time, edit);
    frame(&mut e, &ctx, &mut time, vec![]);

    let close = point(&ctx, "edit-scale");

    click(&mut e, &ctx, &mut time, close);
    assert!(e.edit.is_none());
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    e.edit_selected().unwrap();
    frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::Escape)]);
    assert!(e.edit.is_none());
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    e.edit_selected().unwrap();
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "library-tab"));
    assert!(e.edit.is_none());
    assert!(e.preferences.view.browser_open);
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    e.preferences.view = ViewPreferences::default();
    e.edit_selected().unwrap();
    e.edit.as_mut().unwrap().draft.degrees[3] = 386.3137138648;
    frame(&mut e, &ctx, &mut time, vec![]);
    let apply = point(&ctx, "edit-apply");
    click(&mut e, &ctx, &mut time, apply);
    assert!(e.edit.is_none());
    assert_eq!(shared.snapshot().parameters.position, 0);
    let tuning = shared.snapshot().slots[3]
        .as_ref()
        .unwrap()
        .prepare()
        .unwrap();
    assert!((tuning.hz[64] / tuning.hz[60] - 1.25).abs() < 1e-9);
    assert_eq!(
        shared.snapshot().slots[0],
        inton_core::state::Project::decode(&before).unwrap().slots[0]
    );
}

#[test]
fn draft_audition_is_temporary_and_cancel_restores_project_tuning() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    let original = shared.snapshot().encode().unwrap();
    let original_hz = shared.publication(0.).0;
    e.edit_selected().unwrap();
    e.edit.as_mut().unwrap().draft.degrees[3] = 386.3137138648;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let audition = point(&ctx, "edit-audition");
    click(&mut e, &ctx, &mut time, audition);
    frame(&mut e, &ctx, &mut time, vec![]);
    assert_ne!(shared.publication(0.).0, original_hz);
    assert_eq!(shared.snapshot().encode().unwrap(), original);
    // Hide/close or a host action can turn audition off outside this widget.
    shared.stop_audition();
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(!shared.audition_enabled.load(SeqCst));
    assert!(!e.edit.as_ref().unwrap().audition);
    let audition = point(&ctx, "edit-audition");
    click(&mut e, &ctx, &mut time, audition);
    let cancel = point(&ctx, "edit-cancel");
    click(&mut e, &ctx, &mut time, cancel);
    assert_eq!(shared.publication(0.).0, original_hz);
    assert!(!shared.audition_enabled.load(SeqCst));
}

#[test]
fn last_page_of_large_edit_is_reachable_and_invalid_apply_keeps_the_draft() {
    let shared = Arc::new(Shared::new());
    let preset = Preset::new(
        "Large table".into(),
        format!(
            "Large table\n128\n{}",
            (1..=128)
                .map(|n| format!("{:.1}\n", n as f64 * 100.))
                .collect::<String>()
        ),
        None,
    );
    shared.assign(2, preset).unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    e.inspect_slot(2);
    e.edit_selected().unwrap();
    e.edit.as_mut().unwrap().page = 7;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    frame(&mut e, &ctx, &mut time, vec![]);

    assert_eq!(e.edit.as_ref().unwrap().count, 128);

    let before = shared.snapshot().encode().unwrap();
    e.edit.as_mut().unwrap().draft.degrees[126] = f64::NAN;
    let apply = point(&ctx, "edit-apply");
    click(&mut e, &ctx, &mut time, apply);
    assert!(e.edit.as_ref().unwrap().error.is_some());
    assert_eq!(shared.snapshot().encode().unwrap(), before);
}
