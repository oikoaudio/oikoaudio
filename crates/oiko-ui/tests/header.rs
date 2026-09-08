use egui::{Context, Event, PointerButton, RawInput, Rect, pos2, vec2};
use oiko_ui::chrome::{ProductInfo, about_menu, header};

fn frame(context: &Context, events: Vec<Event>) -> (bool, [egui::Pos2; 2]) {
    let mut open = false;
    let mut targets = [egui::Pos2::ZERO; 2];
    let mut output = context.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 350.0))),
            events,
            ..Default::default()
        },
        |ui| {
            let response = header(ui, "Example", &mut true);
            targets = [response.title_rect.center(), response.anchor.rect.center()];
            open = about_menu(
                &response,
                ProductInfo {
                    name: "Example",
                    version: "1.0",
                    website: "https://oikoaudio.com",
                },
                1.0,
                |_| {},
            )
            .is_open;
        },
    );
    output.textures_delta.clear();
    (open, targets)
}

#[test]
fn title_brand_and_header_right_click_open_the_same_menu() {
    for (target, button) in [
        (0, PointerButton::Primary),
        (1, PointerButton::Primary),
        (0, PointerButton::Secondary),
    ] {
        let context = Context::default();
        oiko_ui::theme::apply_theme(&context, true);
        let (_, targets) = frame(&context, vec![]);
        let position = targets[target];
        frame(&context, vec![Event::PointerMoved(position)]);
        frame(
            &context,
            vec![Event::PointerButton {
                pos: position,
                button,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            frame(
                &context,
                vec![Event::PointerButton {
                    pos: position,
                    button,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }]
            )
            .0
        );
        assert!(
            !frame(
                &context,
                vec![Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]
            )
            .0
        );
    }
}

fn small_canvas_frame(
    context: &Context,
    logical_size: egui::Vec2,
    canvas_scale: f32,
    extra_help: bool,
    open: bool,
    events: Vec<Event>,
) -> (Option<f32>, Vec<(String, Rect)>) {
    let viewport = Rect::from_min_size(egui::Pos2::ZERO, logical_size * canvas_scale);
    let mut selected = None;
    let mut output = context.run_ui(
        RawInput {
            screen_rect: Some(viewport),
            events,
            ..Default::default()
        },
        |root| {
            // Reproduce the macOS canvas transform on any test platform.
            let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("scaled-editor"));
            context
                .set_transform_layer(layer, egui::emath::TSTransform::from_scaling(canvas_scale));
            let mut ui = root.new_child(
                egui::UiBuilder::new()
                    .layer_id(layer)
                    .max_rect(Rect::from_min_size(egui::Pos2::ZERO, logical_size)),
            );
            let mut response = header(&mut ui, "WOW", &mut true);
            response.toggle_about = open;
            selected = about_menu(
                &response,
                ProductInfo {
                    name: "Oiko Wow",
                    version: "0.1.1-beta.3",
                    website: "https://oikoaudio.com",
                },
                canvas_scale,
                |ui| {
                    // Inton's additional help/status text must not displace zoom controls.
                    for _ in 0..if extra_help { 5 } else { 0 } {
                        ui.label("Additional product information and help.");
                    }
                },
            )
            .scale;
        },
    );
    let mut labels = Vec::new();
    for shape in &output.shapes {
        if let egui::Shape::Text(text) = &shape.shape
            && text.galley.job.text.ends_with('%')
        {
            labels.push((
                text.galley.job.text.clone(),
                Rect::from_min_size(text.pos, text.galley.size()),
            ));
        }
    }
    output.textures_delta.clear();
    (selected, labels)
}

#[test]
fn zoom_choices_work_in_small_transformed_canvases() {
    for logical_size in [vec2(500.0, 390.0), vec2(420.0, 350.0)] {
        for extra_help in [false, true] {
            for scale in [0.5, 1.0, 2.0] {
                for target in oiko_ui::scale::UI_SCALE_STEPS {
                    let context = Context::default();
                    oiko_ui::theme::apply_theme(&context, true);
                    small_canvas_frame(&context, logical_size, scale, extra_help, true, vec![]);
                    small_canvas_frame(&context, logical_size, scale, extra_help, false, vec![]);
                    let (_, labels) = small_canvas_frame(
                        &context,
                        logical_size,
                        scale,
                        extra_help,
                        false,
                        vec![],
                    );
                    let label = format!("{}%", (target * 100.0) as u32);
                    let pos = labels
                        .iter()
                        .find(|(text, _)| text == &label)
                        .unwrap()
                        .1
                        .center();
                    small_canvas_frame(
                        &context,
                        logical_size,
                        scale,
                        extra_help,
                        false,
                        vec![Event::PointerMoved(pos)],
                    );
                    let mut selected = None;
                    for pressed in [true, false] {
                        selected = small_canvas_frame(
                            &context,
                            logical_size,
                            scale,
                            extra_help,
                            false,
                            vec![Event::PointerButton {
                                pos,
                                button: PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            }],
                        )
                        .0
                        .or(selected);
                    }
                    assert_eq!(selected, Some(target));
                }
            }
        }
    }
}
