use super::*;

#[test]
fn header_right_click_opens_existing_about_menu() {
    let mut e = Editor::new(Arc::new(Shared::new()), HostRef::disconnected());
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    frame(&mut e, &ctx, &mut time, vec![]);
    let pos = point(&ctx, "header-title");
    for pressed in [true, false] {
        frame(
            &mut e,
            &ctx,
            &mut time,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "listening-guide"));
    assert!(e.overlay == Overlay::ListeningGuide);
}

#[test]
fn guided_collection_survives_empty_favorites_and_opens_from_the_guide() {
    let shared = Arc::new(Shared::new());
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    e.preferences.view.browser_open = false;
    e.preferences.favorites.clear();
    e.search = "no matching scale".into();
    e.section = 2;
    e.overlay = Overlay::ListeningGuide;
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "browse-guided-scales"));
    assert!(!e.overlay.is_guide());
    assert!(e.preferences.view.browser_open);
    assert_eq!(e.category, "Listening guide");
    assert_eq!(e.section, 1);
    assert!(e.search.is_empty());
    assert!(e.preferences.favorites.is_empty());
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    frame(&mut e, &ctx, &mut time, vec![]);

    e.section = 0;
    e.category = "All categories".into();
    e.search = "unmatched".into();
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    click(&mut e, &ctx, &mut time, point(&ctx, "category-menu"));
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "guided-category"));
    assert_eq!(e.category, "Listening guide");
    assert_eq!(e.section, 1);
    assert!(e.search.is_empty() && e.preferences.favorites.is_empty());
    assert_eq!(shared.snapshot().encode().unwrap(), before);
}

#[test]
fn listening_preference_toggles_in_every_inspection_mode_without_changing_project() {
    let lib = Library::factory();
    let shared = Arc::new(Shared::new());
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for entry in &lib.entries {
        let preset = lib.load(&entry.id).unwrap();
        let hint = crate::listening::hint_for(&preset, &preset.prepare().unwrap());
        shared.assign(0, preset.clone()).unwrap();
        shared.assign(1, preset.clone()).unwrap();
        for mode in ["active", "inactive", "preview", "audition"] {
            e.inspection = Inspection::Active;
            e.preview = None;
            e.preferences.view.listening_ideas = false;
            shared.stop_audition();
            if mode == "inactive" {
                e.inspect_slot(1);
            }
            if mode == "preview" || mode == "audition" {
                e.inspection = Inspection::Library(entry.id.clone());
                e.preview = Some(ValidatedScale::new(preset.clone()).unwrap());
            }
            if mode == "audition" {
                shared.audition_enabled.store(true, SeqCst);
                shared.audition_scale(preset.clone()).unwrap();
            }
            let before = shared.snapshot().encode().unwrap();
            for enabled in [false, true, false] {
                frame(&mut e, &ctx, &mut time, vec![]);
                if !hint.is_empty() && e.preferences.view.listening_ideas != enabled {
                    let bulb = point(&ctx, "listening-ideas");
                    click(&mut e, &ctx, &mut time, bulb);
                }
                frame(&mut e, &ctx, &mut time, vec![]);
                if !hint.is_empty() {
                    assert_eq!(e.preferences.view.listening_ideas, enabled);
                }
                assert_eq!(shared.snapshot().encode().unwrap(), before);
            }
        }
    }
}

#[test]
fn listening_controls_toggle_preference_and_guide_without_changing_project() {
    let shared = Arc::new(Shared::new());
    let preset = Library::factory().load("factory:22edo").unwrap();
    shared.assign(0, preset).unwrap();
    let before = shared.snapshot().encode().unwrap();
    let mut e = Editor::new(shared.clone(), HostRef::disconnected());
    e.preferences.view = ViewPreferences::default();
    settle(&mut e);
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    frame(&mut e, &ctx, &mut time, vec![]);

    let bulb = point(&ctx, "listening-ideas");
    click(&mut e, &ctx, &mut time, bulb);
    assert!(e.preferences.view.listening_ideas);
    frame(&mut e, &ctx, &mut time, vec![]);

    click(&mut e, &ctx, &mut time, point(&ctx, "brand-menu"));
    frame(&mut e, &ctx, &mut time, vec![]);
    let guide = point(&ctx, "listening-guide");
    click(&mut e, &ctx, &mut time, guide);
    assert!(e.overlay.is_guide());
    frame(&mut e, &ctx, &mut time, vec![]);
    frame(
        &mut e,
        &ctx,
        &mut time,
        vec![key_event(egui::Key::ArrowRight)],
    );
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::Escape)]);
    assert!(!e.overlay.is_guide());
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "brand-menu"));
    for _ in 0..3 {
        frame(&mut e, &ctx, &mut time, vec![]);
    }
    let guide = point(&ctx, "listening-guide");
    click(&mut e, &ctx, &mut time, guide);
    frame(&mut e, &ctx, &mut time, vec![]);
    assert!(e.overlay.is_guide());
    click(&mut e, &ctx, &mut time, pos2(20., 450.));
    assert!(!e.overlay.is_guide());
    frame(&mut e, &ctx, &mut time, vec![]);
    click(&mut e, &ctx, &mut time, point(&ctx, "listening-ideas"));
    frame(&mut e, &ctx, &mut time, vec![key_event(egui::Key::Escape)]);
    assert!(!e.preferences.view.listening_ideas);
    frame(&mut e, &ctx, &mut time, vec![]);
    assert_eq!(shared.snapshot().encode().unwrap(), before);
}
