use super::*;

#[test]
fn blocked_warning_can_be_cancelled_without_resetting_or_hiding_status() {
    let shared = Arc::new(Shared::new());
    *shared.status.lock().unwrap() = (MasterStatus::BlockedByOtherMaster, 0);
    shared.recovery_available.store(true, SeqCst);
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    for dismiss in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);

        click(&mut e, &ctx, &mut time, point(&ctx, "connection-status"));
        frame(&mut e, &ctx, &mut time, vec![]);

        match dismiss {
            0 => click(&mut e, &ctx, &mut time, point(&ctx, "cancel-connection")),
            1 => {
                frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::Escape)]);
            }
            _ => click(&mut e, &ctx, &mut time, pos2(20., 450.)),
        }
        assert!(e.overlay != Overlay::Connection);
        assert!(!shared.recovery_requested.load(SeqCst));
        assert_eq!(shared.snapshot().encode().unwrap(), before);
    }
    shared.recovery_available.store(false, SeqCst);
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "connection-status"));
    assert!(e.overlay == Overlay::Connection);
    frame(&mut e, &ctx, &mut time, vec![]);
}

#[test]
fn master_recovery_needs_a_deliberate_action_and_preserves_project() {
    let shared = Arc::new(Shared::new());
    *shared.status.lock().unwrap() = (MasterStatus::BlockedByOtherMaster, 0);
    shared.recovery_available.store(true, SeqCst);
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "connection-status"));
    assert!(e.overlay == Overlay::Connection);
    assert!(!shared.recovery_requested.load(SeqCst));
    frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::Escape)]);
    assert!(e.overlay != Overlay::Connection);
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "connection-status"));
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "recover-connection"));
    assert!(shared.recovery_requested.load(SeqCst));
    assert!(e.overlay != Overlay::Connection);
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    shared.recovery_requested.store(false, SeqCst);
    shared.recovery_available.store(false, SeqCst);
    e.overlay = Overlay::Connection;
    frame(&mut e, &ctx, &mut time, vec![]);
}

#[test]
fn cancelling_file_selection_is_silent_but_dialog_failures_are_reported() {
    assert_eq!(dialog_selection(Some(1), vec![]), Ok(None));
    assert_eq!(dialog_selection(Some(0), b"\n".to_vec()), Ok(None));
    assert_eq!(
        dialog_selection(Some(0), b"/tmp/my scale.scl\n".to_vec()),
        Ok(Some(PathBuf::from("/tmp/my scale.scl")))
    );
    assert!(dialog_selection(Some(2), vec![]).is_err());
    assert!(dialog_selection(None, vec![]).is_err());
}

#[test]
fn folder_settings_dismiss_with_escape_and_outside_click_without_applying() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    let original_folder = e.preferences.folder.clone();
    let original_project = shared.snapshot().encode().unwrap();
    e.folder_text = "/tmp/unapplied-folder-edit".into();
    e.preferences.view = ViewPreferences::default();
    e.preferences.view.browser_open = true;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let gear = point(&ctx, "folder-settings");
    click(&mut e, &ctx, &mut time, gear);
    assert!(
        e.overlay == Overlay::Settings,
        "Gear opens settings and does not immediately dismiss them"
    );
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let folder = point(&ctx, "folder-path");
    click(&mut e, &ctx, &mut time, folder);
    assert!(
        e.overlay == Overlay::Settings,
        "Clicking inside should keep settings open"
    );
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(
        e.overlay != Overlay::Settings,
        "Escape should dismiss folder settings"
    );
    e.overlay = Overlay::Settings;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let browse = point(&ctx, "browse-chevron");
    click(&mut e, &ctx, &mut time, browse);
    assert!(
        e.preferences.view.browser_open,
        "Dismissal must not click through to Browse"
    );
    assert!(
        e.overlay != Overlay::Settings,
        "An outside click should dismiss folder settings"
    );
    assert_eq!(e.preferences.folder, original_folder);
    assert_eq!(shared.snapshot().encode().unwrap(), original_project);
}
