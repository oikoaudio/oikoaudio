use super::*;

#[test]
fn opening_library_focuses_search_and_vertical_keys_resume_list_cursor() {
    let shared = Arc::new(Shared::new());
    shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    e.preferences.view.browser_open = false;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "library-tab"));
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(ctx.text_edit_focused());
    assert_eq!(e.browser_cursor.as_deref(), Some("factory:12edo"));
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowDown)],
    );
    settle(&mut e);
    assert!(!ctx.text_edit_focused());
    let first = e.browser_cursor.clone();
    assert!(first.is_some());
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowDown)],
    );
    settle(&mut e);
    let second = e.browser_cursor.clone();
    assert_ne!(first, second);
    click(&mut e, &ctx, &mut time, point(&ctx, "library-tab"));
    click(&mut e, &ctx, &mut time, point(&ctx, "library-tab"));
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(ctx.text_edit_focused());
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowDown)],
    );
    settle(&mut e);
    assert_ne!(e.browser_cursor, second);
}

#[test]
fn library_title_arrows_advance_selection_in_both_modes() {
    for multiple in [false, true] {
        let shared = Arc::new(Shared::new());
        if multiple {
            shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
        }
        let mut e = Editor::new(shared.clone(), HostRef::disconnected());
        settle(&mut e);
        e.preferences.view.browser_open = true;
        e.browser_tab = BrowserTab::Library;
        e.section = 1;
        let ctx = egui::Context::default();
        let mut time = 0.;
        for _ in 0..3 {
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        for _ in 0..20 {
            click(&mut e, &ctx, &mut time, point(&ctx, "next-scale"));
            settle(&mut e);
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        assert!(e.library_scroll > 0.);

        if multiple {
            assert_eq!(shared.publication(0.).1, "12 EDO");
            assert!(e.preview.is_some());
        } else {
            assert!(e.preview.is_none());
            assert_eq!(
                shared.snapshot().slots[0].as_ref().unwrap().source_id,
                e.browser_cursor
            );
        }
    }
}

#[test]
fn category_selection_keeps_browser_open() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    let mut library = Library::factory();
    for (i, entry) in library.entries.iter_mut().enumerate() {
        entry.category = if i % 8 == 7 {
            "User/Derived cycles".into()
        } else {
            format!("Category {}", i % 8)
        };
    }
    e.library = Some(Arc::new(library));
    e.section = 3;
    e.preferences.view.browser_open = true;
    e.browser_tab = BrowserTab::Library;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "category-menu"));
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let id = egui::Id::new("category-option:User/Derived cycles");
    let before = ctx.data(|d| d.get_temp::<egui::Rect>(id)).unwrap();
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(before.center())],
    );
    let after = ctx.data(|d| d.get_temp::<egui::Rect>(id)).unwrap();

    click(&mut e, &ctx, &mut time, after.center());
    assert_eq!(e.category, "User/Derived cycles");
    assert!(e.preferences.view.browser_open);
}

#[test]
fn source_icons_toggle_back_to_all() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    e.preferences.view.browser_open = true;
    e.browser_tab = BrowserTab::Library;
    e.section = 3;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for kind in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
        let at = point(&ctx, &format!("source-filter:{kind}"));
        click(&mut e, &ctx, &mut time, at);
        assert_eq!(e.section, kind);
        click(&mut e, &ctx, &mut time, at);
        assert_eq!(e.section, 3);
    }
}

#[test]
fn browser_closes_by_selected_icon_tab_and_outside_click() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for target in ["browse-chevron", "library-tab", "project-tab", "outside"] {
        e.preferences.view.browser_open = true;
        e.browser_tab = if target == "project-tab" {
            BrowserTab::Set
        } else {
            BrowserTab::Library
        };
        for _ in 0..3 {
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        let pos = if target == "outside" {
            pos2(5., 450.)
        } else {
            point(&ctx, target)
        };
        click(&mut e, &ctx, &mut time, pos);
        assert!(
            !e.preferences.view.browser_open,
            "{target} must close the browser"
        );
    }
}

#[test]
fn library_assignment_requires_selection_and_an_explicit_action() {
    let shared = Arc::new(Shared::new());
    shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    e.preferences.view.browser_open = true;
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    assert!(
        ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("assign:factory:22edo")))
            .is_none()
    );
    click(&mut e, &ctx, &mut time, point(&ctx, "scale:factory:22edo"));
    e.show_project();
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "replace-slot:0"));
    assert!(matches!(e.assignment, AssignmentIntent::Replace(_)));
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "scale:factory:22edo"));
    frame(&mut e, &ctx, &mut time, vec![]);

    click(&mut e, &ctx, &mut time, point(&ctx, "assign:factory:22edo"));
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
    e.show_project();
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "add-scale"));
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "scale:factory:22edo"));
    frame(&mut e, &ctx, &mut time, vec![]);

    click(&mut e, &ctx, &mut time, point(&ctx, "assign:factory:22edo"));
    settle(&mut e);
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 3);
}

#[test]
fn library_choice_loads_single_scale_without_preview() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.choose_library_scale("factory:22edo".into());
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert!(e.inspection.library().is_none() && e.preview.is_none());
    assert!(!shared.audition_enabled.load(SeqCst));
    e.begin_add_scale();
    e.choose_library_scale("factory:19edo".into());
    settle(&mut e);
    assert_eq!(e.preview.as_ref().unwrap().preset().display_name, "19 EDO");
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
}

#[test]
fn unused_host_positions_do_not_create_rows_or_change_tuning() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    frame(&mut e, &ctx, &mut time, vec![]);
    let slots = shared.snapshot().slots;
    let tuning = shared.publication(0.).0;
    for position in [31, 7, 16, 31] {
        let mut parameters = shared.parameters.read().unwrap();
        parameters.position = position;
        shared.parameters.write(parameters);
        assert_eq!(shared.publication(0.).0, tuning);
        for _ in 0..3 {
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        assert_eq!(e.set.rows(&shared.snapshot(), position), vec![0]);

        assert_eq!(shared.snapshot().slots, slots);
    }
}

#[test]
fn failed_preview_shows_error_and_recovers_on_next_selection() {
    let shared = Arc::new(Shared::new());
    let original = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view.browser_open = true;
    settle(&mut e);
    e.select("missing-scale".into());
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    frame(&mut e, &ctx, &mut time, vec![]);

    assert!(e.preview_error.is_some());
    assert!(e.preview.is_none());
    assert!(e.job.is_none());
    assert_eq!(shared.snapshot().encode().unwrap(), original);
    e.select("factory:22edo".into());
    settle(&mut e);
    frame(&mut e, &ctx, &mut time, vec![]);

    assert!(e.preview_error.is_none());
    assert!(e.preview.is_some());
    assert!(e.job.is_none());
}

#[test]
fn browser_arrows_preview_filtered_rows_and_assign_the_destination() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    e.section = 1;
    e.search = "detuned".into();
    e.set.destination = 3;
    e.preferences.view.browser_open = true;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowDown)],
    );
    settle(&mut e);
    assert!(
        e.inspection
            .library()
            .is_some_and(|id| id.starts_with("factory:detune-"))
    );
    let first = e.inspection.library().cloned();
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowDown)],
    );
    settle(&mut e);
    assert_ne!(e.inspection.library(), first.as_ref());
    frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::ArrowUp)]);
    settle(&mut e);
    assert_eq!(e.inspection.library(), first.as_ref());
    assert!(shared.snapshot().slots[3].is_none());
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowRight)],
    );
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[3].as_ref().unwrap().source_id,
        first
    );
    assert_eq!(shared.snapshot().parameters.position, 0);
}

#[test]
fn browser_keys_preserve_text_entry_and_scroll_to_later_results() {
    let shared = Arc::new(Shared::new());
    shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    e.section = 1;
    let original = shared.snapshot().encode().unwrap();
    e.preferences.view.browser_open = true;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let search = point(&ctx, "library-search");
    click(&mut e, &ctx, &mut time, search);
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowRight)],
    );
    assert!(e.inspection.library().is_none());
    assert_eq!(shared.snapshot().encode().unwrap(), original);
    ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    let rows = e.filtered_scales(e.library.as_ref().unwrap());
    let count = rows.len();
    let last = rows.last().unwrap().id.clone();
    for _ in 0..=count {
        frame(
            &mut e,
            &ctx,
            &mut time,
            vec![key_event(egui::Key::ArrowDown)],
        );
    }
    settle(&mut e);
    frame(&mut e, &ctx, &mut time, vec![]);
    assert_eq!(e.inspection.library(), Some(&last));

    assert!(e.library_scroll > 0.);
    assert_eq!(shared.snapshot().encode().unwrap(), original);
}

#[test]
fn escape_closes_browser_with_search_focused() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences {
        browser_open: true,
        ..ViewPreferences::default()
    };
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "library-search"));
    frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::Escape)]);
    assert!(!e.preferences.view.browser_open);
}

#[test]
fn browse_button_and_ctrl_b_toggle_without_changing_project() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let browse = point(&ctx, "browse-chevron");

    click(&mut e, &ctx, &mut time, browse);
    assert!(e.preferences.view.browser_open);

    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(
        ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("source-filter:0")))
            .is_some()
    );

    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::Key {
            key: egui::Key::B,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        }],
    );
    assert!(!e.preferences.view.browser_open);
    assert_eq!(shared.snapshot().parameters.position, 0);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "12 EDO"
    );
}

#[test]
fn preview_is_silent_and_queued_assignments_capture_their_destination() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    let before = shared.snapshot();
    e.select("factory:22edo".into());
    settle(&mut e);
    assert_eq!(
        shared.snapshot().encode().unwrap(),
        before.encode().unwrap()
    );
    e.assign_scale("factory:13ed3".into(), 7);
    e.set.destination = 2;
    e.assign_scale("factory:22edo".into(), 2);
    settle(&mut e);
    let after = shared.snapshot();
    assert_eq!(after.parameters.position, 0);
    assert_eq!(
        after.slots[7].as_ref().unwrap().prepare().unwrap().count,
        13
    );
    assert_eq!(
        after.slots[2].as_ref().unwrap().prepare().unwrap().count,
        22
    );
    e.assign_scale("not-a-scale".into(), 7);
    settle(&mut e);
    assert_eq!(shared.snapshot().encode().unwrap(), after.encode().unwrap());
    e.assign_scale("factory:22edo".into(), 0);
    settle(&mut e);
    assert_eq!(shared.project.lock().unwrap().engine.name, "22 EDO");
}
