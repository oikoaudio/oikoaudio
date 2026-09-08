use super::*;
#[test]
fn malformed_and_out_of_range_preferences_are_safe() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(nearest_scale(value), 1.0);
    }
    assert_eq!(nearest_scale(-9.0), 0.5);
    assert_eq!(nearest_scale(100.0), 2.0);
    assert_eq!(nearest_scale(1.13), 1.25);
    for step in SCALE_STEPS {
        assert_eq!(nearest_scale(step), step);
    }
}

#[test]
fn restored_zoom_and_menu_changes_request_the_same_geometry() {
    for canvas in [egui::vec2(760.0, 560.0), egui::vec2(640.0, 400.0)] {
        for scale in UI_SCALE_STEPS {
            let mut results = Vec::new();
            for apply in [initialize_scale, request_scale] {
                let context = egui::Context::default();
                apply(&context, scale, canvas);
                let mut output = context.run_ui(egui::RawInput::default(), |_| {});
                output.textures_delta.clear();
                let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
                let size = commands.iter().find_map(|command| match command {
                    egui::ViewportCommand::InnerSize(size) => Some(*size),
                    _ => None,
                });
                if cfg!(target_os = "macos") {
                    assert_eq!(size, Some(canvas * scale));
                    assert_eq!(context.zoom_factor(), 1.0);
                } else {
                    assert_eq!(context.zoom_factor(), scale);
                }
                results.push((size, context.zoom_factor()));
            }
            assert_eq!(results[0], results[1]);
        }
    }
}
