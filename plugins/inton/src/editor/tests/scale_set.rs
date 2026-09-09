use super::*;

#[test]
fn visible_history_and_set_only_clear_restore_slots() {
    let shared = Arc::new(Shared::new());
    shared
        .append_scales(vec![
            Library::factory().load("factory:19edo").unwrap(),
            Library::factory().load("factory:22edo").unwrap(),
        ])
        .unwrap();
    let before = shared.snapshot();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view.browser_open = true;
    e.browser_tab = BrowserTab::Set;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for (action, count) in [("clear-scale-set", 1), ("Undo", 3), ("Redo", 1)] {
        for _ in 0..3 {
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        for _ in 0..3 {
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        click(&mut e, &ctx, &mut time, point(&ctx, action));
        assert_eq!(shared.snapshot().slots.iter().flatten().count(), count);
        if action == "Undo" {
            assert_eq!(shared.snapshot().slots, before.slots);
        }
    }
}

#[test]
fn project_tab_opens_the_set_and_toggles_the_browser() {
    let shared = Arc::new(Shared::new());
    shared
        .assign(1, Library::factory().load("factory:19edo").unwrap())
        .unwrap();
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    e.preferences.view.browser_open = true;
    e.browser_tab = BrowserTab::Library;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "project-tab"));
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(e.browser_tab == BrowserTab::Set);
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "project-tab"));
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(!e.preferences.view.browser_open);
}

#[test]
fn project_tabs_preserve_tuning_state() {
    let shared = Arc::new(Shared::new());
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
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
    click(&mut e, &ctx, &mut time, point(&ctx, "project-tab"));
    assert!(e.browser_tab == BrowserTab::Set);

    assert_eq!(shared.snapshot().encode().unwrap(), before);
    frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::Escape)]);
    assert!(!e.preferences.view.browser_open);
}

#[test]
fn replacement_requires_explicit_slot_action_and_is_one_shot() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    shared.assign(3, inton_core::tuning::twelve_edo()).unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.inspect_slot(0);
    e.assign_from_library("factory:22edo".into(), 0);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "12 EDO"
    );
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "22 EDO"
    );
    e.begin_replace_scale(0);
    e.assign_from_library("factory:19edo".into(), 0);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "19 EDO"
    );
    assert!(!matches!(e.assignment, AssignmentIntent::Replace(_)));
    e.assign_from_library("factory:31edo".into(), 0);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "19 EDO"
    );
    assert_eq!(
        shared.snapshot().slots[2].as_ref().unwrap().display_name,
        "31 EDO"
    );
    shared.clear(1).unwrap();
    e.inspect_slot(1);
    assert!(!matches!(e.assignment, AssignmentIntent::Replace(_)));
    e.assign_from_library("factory:7edo".into(), 1);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "7 EDO"
    );
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 4);
}

#[test]
fn single_tuning_loads_in_place_and_plus_enters_persistent_set_mode() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    frame(&mut e, &ctx, &mut time, vec![]);

    assert!(!shared.snapshot().uses_scale_set());
    e.assign_from_library("factory:22edo".into(), 0);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 1);
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "project-tab"));
    frame(&mut e, &ctx, &mut time, vec![]);

    assert!(e.browser_tab == BrowserTab::Set);
    frame(&mut e, &ctx, &mut time, vec![]);

    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 1);
    e.begin_add_scale();
    e.assign_from_library("factory:19edo".into(), 0);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "19 EDO"
    );
    shared.clear(1).unwrap();
    let recalled = Project::decode(&shared.snapshot().encode().unwrap()).unwrap();
    assert!(recalled.uses_scale_set());
    shared.restore(recalled).unwrap();
    e.inspect_slot(0);
    e.assign_from_library("factory:31edo".into(), 0);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "31 EDO"
    );
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 1);
}

#[test]
fn scale_set_opens_without_browser() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences {
        browser_open: false,
        ..ViewPreferences::default()
    };
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "project-tab"));
    frame(&mut e, &ctx, &mut time, vec![]);

    assert!(e.browser_tab == BrowserTab::Set);
    assert!(e.preferences.view.browser_open);
}

#[test]
fn title_steps_follow_filters_and_never_overwrite_set_slots() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.section = 1;
    e.search = "22 EDO".into();
    e.step_scale(true);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
    shared.enable_scale_set();
    shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    let before = shared.snapshot().encode().unwrap();
    e.search = "19 EDO".into();
    e.step_scale(true);
    settle(&mut e);
    assert!(e.preview.is_none());
    assert_eq!(
        shared
            .take_parameter_edits()
            .find(|(id, _)| *id == Parameter::Position)
            .unwrap()
            .1,
        1.
    );
    assert_eq!(
        shared.snapshot().slots,
        Project::decode(&before).unwrap().slots
    );
    e.search = "no matching tuning".into();
    assert!(e.step_target(true).is_none());
}

#[test]
fn arrows_load_the_only_scale_even_when_set_mode_is_enabled() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.section = 1;
    e.search = "22 EDO".into();
    e.step_scale(true);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(shared.publication(0.).1, "22 EDO");
    assert!(e.inspection.library().is_none() && e.preview.is_none());
}

#[test]
fn slot_arrows_activate_across_gaps_in_folded_and_open_views() {
    for folded in [false, true] {
        let shared = Arc::new(Shared::new());
        shared
            .assign(3, Library::factory().load("factory:19edo").unwrap())
            .unwrap();
        shared.fold_scale_set(folded);
        let mut e = Editor::new(shared.clone(), HostRef::disconnected());
        settle(&mut e);
        e.show_project();
        let ctx = egui::Context::default();
        let mut time = 0.;
        for _ in 0..3 {
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        click(&mut e, &ctx, &mut time, point(&ctx, "next-scale"));
        assert_eq!(
            shared
                .take_parameter_edits()
                .find(|(id, _)| *id == Parameter::Position)
                .unwrap()
                .1,
            3.
        );
        let mut parameters = shared.parameters.read().unwrap();
        parameters.position = 3;
        shared.parameters.write(parameters);
        assert_eq!(shared.snapshot().parameters.position, 3);
        assert_eq!(shared.publication(0.).1, "19 EDO");
        for _ in 0..3 {
            frame(&mut e, &ctx, &mut time, vec![]);
        }
        click(&mut e, &ctx, &mut time, point(&ctx, "previous-scale"));
        assert_eq!(
            shared
                .take_parameter_edits()
                .find(|(id, _)| *id == Parameter::Position)
                .unwrap()
                .1,
            0.
        );
        let mut parameters = shared.parameters.read().unwrap();
        parameters.position = 0;
        shared.parameters.write(parameters);
        assert_eq!(shared.snapshot().parameters.position, 0);
        assert_eq!(shared.snapshot().set_collapsed, folded);
    }
}

#[test]
fn plus_row_opens_library_without_creating_a_slot_then_arrow_appends() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    e.preferences.view.browser_open = false;
    settle(&mut e);
    e.show_project();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    frame(&mut e, &ctx, &mut time, vec![]);

    click(&mut e, &ctx, &mut time, point(&ctx, "add-scale"));
    assert!(e.preferences.view.browser_open);
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    assert_eq!(e.set.rows(&shared.snapshot(), 0), vec![0]);
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    assert!(ctx.text_edit_focused());
    click(&mut e, &ctx, &mut time, point(&ctx, "scale:factory:22edo"));
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "assign:factory:22edo"));
    settle(&mut e);
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 2);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "12 EDO"
    );
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(shared.snapshot().parameters.position, 0);
}

#[test]
fn add_target_routes_double_click_keyboard_and_editor_to_new_slots() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    e.begin_add_scale();
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let source = point(&ctx, "scale:factory:19edo");
    click(&mut e, &ctx, &mut time, source);
    settle(&mut e);
    click(&mut e, &ctx, &mut time, source);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "19 EDO"
    );
    e.begin_add_scale();
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    e.search = "31 EDO".into();
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "scale:factory:31edo"));
    settle(&mut e);
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowRight)],
    );
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[2].as_ref().unwrap().display_name,
        "31 EDO"
    );
    e.begin_add_scale();
    e.select("factory:22edo".into());
    settle(&mut e);
    e.edit_selected().unwrap();
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "edit-apply"));
    assert_eq!(
        shared.snapshot().slots[3].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "12 EDO"
    );
    assert_eq!(shared.snapshot().parameters.position, 0);
    e.begin_add_scale();
    e.begin_replace_scale(1);
    e.assign_from_library("factory:13ed3".into(), 1);
    settle(&mut e);
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 4);
    assert_eq!(
        shared.snapshot().slots[1]
            .as_ref()
            .unwrap()
            .prepare()
            .unwrap()
            .count,
        13
    );
}

#[test]
fn dropping_on_plus_appends_and_a_full_set_has_no_add_row() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    e.preferences.view.browser_open = true;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    drag(
        &mut e,
        &ctx,
        &mut time,
        point(&ctx, "scale:factory:22edo"),
        point(&ctx, "project-tab"),
    );
    settle(&mut e);
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 2);
    for n in 2..32 {
        shared.assign(n, inton_core::tuning::twelve_edo()).unwrap();
    }
    e.show_project();
    // Remove stored trace so absence is meaningful on the next frame.
    ctx.data_mut(|d| {
        d.remove::<egui::Rect>(egui::Id::new("add-scale"));
    });
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(
        ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("add-scale")))
            .is_none()
    );
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 32);
    shared.clear(1).unwrap();
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(
        ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("add-scale")))
            .is_none()
    );
}

#[test]
fn dropping_library_scale_into_set_space_adds_without_plus() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    e.preferences.view.browser_open = true;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let source = point(&ctx, "scale:factory:22edo");
    let target = point(&ctx, "project-tab");
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(source), pointer(source, true)],
    );
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(source + vec2(15., 0.))],
    );
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(target)],
    );
    frame(&mut e, &ctx, &mut time, vec![]);
    assert_eq!(
        ctx.data(|d| d.get_temp::<bool>(egui::Id::new("project-drop-hover"))),
        Some(true)
    );
    assert!(e.browser_tab != BrowserTab::Set);
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 1);
    frame(&mut e, &ctx, &mut time, vec![pointer(target, false)]);
    frame(&mut e, &ctx, &mut time, vec![]);
    assert_eq!(
        ctx.data(|d| d.get_temp::<bool>(egui::Id::new("project-drop-hover"))),
        Some(false)
    );
    settle(&mut e);
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 2);
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(shared.publication(0.).1, "12 EDO");
    assert!(e.browser_tab != BrowserTab::Set);
}

#[test]
fn selecting_inactive_set_row_shows_embedded_scale_without_activation() {
    let shared = Arc::new(Shared::new());
    let mut preset = Library::factory().load("factory:13ed3").unwrap();
    preset.source_id = Some("missing-original-file".into());
    shared.assign(1, preset).unwrap();
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    e.select("factory:22edo".into());
    settle(&mut e);
    e.show_project();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let slot = point(&ctx, "slot:1");
    click(&mut e, &ctx, &mut time, slot);
    frame(&mut e, &ctx, &mut time, vec![]);

    assert_eq!(e.inspection.slot(), Some(1));
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    let mut p = shared.parameters.read().unwrap();
    p.position = 1;
    shared.parameters.write(p);
    frame(&mut e, &ctx, &mut time, vec![]);
}

#[test]
fn dragging_set_rows_reorders_and_keeps_sounding_scale_active() {
    let shared = Arc::new(Shared::new());
    shared
        .assign(1, Library::factory().load("factory:22edo").unwrap())
        .unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    e.show_project();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let from = point(&ctx, "slot:0");
    let to = point(&ctx, "slot:1") + vec2(0., 8.);
    drag(&mut e, &ctx, &mut time, from, to);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(shared.snapshot().parameters.position, 1);
    assert_eq!(shared.publication(0.).1, "12 EDO");
}

#[test]
fn queued_appends_allocate_distinct_slots_and_cancelled_drags_do_nothing() {
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    e.enqueue(Command::Append("factory:22edo".into()));
    e.enqueue(Command::Append("factory:13ed3".into()));
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(
        shared.snapshot().slots[2].as_ref().unwrap().display_name,
        "Bohlen-Pierce · 13 ED3"
    );
    let before = shared.snapshot().encode().unwrap();
    e.preferences.view.browser_open = true;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..5 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    e.show_project();
    frame(&mut e, &ctx, &mut time, vec![]);
    let from = point(&ctx, "slot:0");
    let to = point(&ctx, "slot:1");
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(from), pointer(from, true)],
    );
    frame(&mut e, &ctx, &mut time, vec![egui::Event::PointerMoved(to)]);
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
    frame(&mut e, &ctx, &mut time, vec![pointer(to, false)]);
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    e.browser_tab = BrowserTab::Library;
    e.preferences.view.browser_open = true;
    frame(&mut e, &ctx, &mut time, vec![]);
    let from = point(&ctx, "scale:factory:22edo");
    drag(&mut e, &ctx, &mut time, from, pos2(30., 30.));
    settle(&mut e);
    assert_eq!(shared.snapshot().encode().unwrap(), before);
}

#[test]
fn removing_last_active_slot_holds_tuning_and_allows_adding_again() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let tuning = shared.publication(0.).0;
    e.preferences.view.browser_open = true;
    e.show_project();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let clear = point(&ctx, "clear:0");
    click(&mut e, &ctx, &mut time, clear);
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(!shared.snapshot().uses_scale_set());
    assert_eq!(shared.publication(0.).0, tuning);
    let recalled =
        inton_core::state::Project::decode(&shared.snapshot().encode().unwrap()).unwrap();
    assert!(recalled.has_slot(0));
    assert!(!recalled.uses_scale_set());

    e.browser_tab = BrowserTab::Library;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "scale:factory:22edo"));
    frame(&mut e, &ctx, &mut time, vec![]);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "22 EDO"
    );
}

#[test]
fn clearing_an_added_slot_keeps_it_selected_for_replacement() {
    let shared = Arc::new(Shared::new());
    shared
        .append(Library::factory().load("factory:19edo").unwrap())
        .unwrap();
    shared
        .append(Library::factory().load("factory:22edo").unwrap())
        .unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    e.inspect_slot(1);
    e.preferences.view.browser_open = true;
    e.show_project();
    assert_eq!(e.inspection.slot(), None);
    e.inspect_slot(1);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let clear = point(&ctx, "clear:1");
    click(&mut e, &ctx, &mut time, clear);
    frame(&mut e, &ctx, &mut time, vec![]);

    assert_eq!(e.inspection.slot(), Some(1));
    assert_eq!(e.set.destination, 1);
    assert_eq!(e.set.rows(&shared.snapshot(), 0), vec![0, 1, 2]);
    e.browser_tab = BrowserTab::Library;
    e.search = "31 EDO".into();
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "scale:factory:31edo"));
    frame(&mut e, &ctx, &mut time, vec![]);
    let arrow = point(&ctx, "assign:factory:31edo");
    click(&mut e, &ctx, &mut time, arrow);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[1].as_ref().unwrap().display_name,
        "31 EDO"
    );
    assert_eq!(
        shared.snapshot().slots[2].as_ref().unwrap().display_name,
        "22 EDO"
    );
    assert_eq!(shared.snapshot().parameters.position, 0);
}

#[test]
fn empty_selected_slot_can_be_dismissed_without_renumbering() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    e.set.revealed.insert(4);
    e.inspect_slot(4);
    e.show_project();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let clear = point(&ctx, "clear:4");
    click(&mut e, &ctx, &mut time, clear);
    frame(&mut e, &ctx, &mut time, vec![]);

    assert_eq!(shared.snapshot().parameters.position, 0);
    assert_eq!(e.set.destination, 0);
}

#[test]
fn empty_unselected_revealed_slot_can_be_dismissed() {
    let shared = Arc::new(Shared::new());
    shared.enable_scale_set();
    let mut e = Editor::new(shared, HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    e.set.revealed.insert(4);
    e.show_project();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let clear = point(&ctx, "clear:4");
    click(&mut e, &ctx, &mut time, clear);
    frame(&mut e, &ctx, &mut time, vec![]);

    assert!(!e.set.revealed.contains(&4));
    assert!(!e.set.rows(&e.shared.snapshot(), 0).contains(&4));
}

#[test]
fn pointer_double_click_drag_and_clear_use_real_widgets() {
    let shared = Arc::new(Shared::new());
    shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    shared.enable_scale_set();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    settle(&mut e);
    e.preferences.view = ViewPreferences::default();
    e.show_project();
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "replace-slot:0"));
    assert!(matches!(e.assignment, AssignmentIntent::Replace(_)));
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    frame(&mut e, &ctx, &mut time, vec![]);
    time += 1.;
    let source = point(&ctx, "scale:factory:22edo") - vec2(45., 0.);
    click(&mut e, &ctx, &mut time, source);
    settle(&mut e);
    assert_eq!(shared.project.lock().unwrap().engine.name, "12 EDO");
    assert!(matches!(e.assignment, AssignmentIntent::Replace(_)));
    assert_eq!(
        e.inspection.library().map(String::as_str),
        Some("factory:22edo")
    );
    click(&mut e, &ctx, &mut time, source);
    settle(&mut e);
    assert_eq!(shared.project.lock().unwrap().engine.name, "22 EDO");
    e.set.add_slot(&shared.snapshot());
    for _ in 0..5 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let source = point(&ctx, "scale:factory:7edo");
    let target = point(&ctx, "project-tab");
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(source), pointer(source, true)],
    );
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(source + vec2(15., 0.))],
    );
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![egui::Event::PointerMoved(target)],
    );
    frame(&mut e, &ctx, &mut time, vec![pointer(target, false)]);
    settle(&mut e);
    assert_eq!(
        shared.snapshot().slots[2].as_ref().unwrap().display_name,
        "7 EDO"
    );
    assert_eq!(shared.project.lock().unwrap().engine.name, "22 EDO");
    frame(&mut e, &ctx, &mut time, vec![]);
    e.show_project();
    frame(&mut e, &ctx, &mut time, vec![]);
    let remove = point(&ctx, "clear:2");

    click(&mut e, &ctx, &mut time, remove);
    assert!(shared.snapshot().slots[2].is_none());
    assert_eq!(shared.project.lock().unwrap().engine.name, "22 EDO");
}
