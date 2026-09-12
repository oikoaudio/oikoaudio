use super::*;
use nice_plug::context::{PluginApi, gui::GuiContextInner};
use nice_plug::params::{InternalParamMut, internals::ParamPtr};
use nice_plug::prelude::PluginState;

struct TestHost;
impl GuiContextInner for TestHost {
    fn request_restart(&self) {}

    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, value: f32) {
        unsafe {
            param._internal_set_normalized_value(value);
        }
    }
    unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
    fn get_state(&self) -> PluginState {
        panic!("Unexpected host state request")
    }
    fn set_state(&self, _: PluginState) {
        panic!("Unexpected host state restore")
    }
}

struct Harness {
    context: egui::Context,
    editor: WowEditor,
}
impl Harness {
    fn new(dark: bool, scale: f32) -> Self {
        let context = egui::Context::default();
        apply_theme(&context, dark);
        context.set_pixels_per_point(scale);
        let params = Arc::new(WowParams::default());
        let mut editor = WowEditor::new(params, Arc::new(ModulationDisplay::default()));
        editor.dark.store(dark, Ordering::Relaxed);
        editor.gui_context = Some(GuiContext::new(Arc::new(TestHost)));
        Self { context, editor }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT));
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                events,
                ..Default::default()
            },
            |root| self.editor.draw_ui(root),
        );
        output.textures_delta.clear();
        output
    }
    fn click(&mut self, id: Id) {
        let pos = self
            .context
            .read_response(id)
            .expect("control exists")
            .rect
            .center();
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
    }
}

#[test]
fn unit_selectors_convert_each_rate_independently_in_both_directions() {
    for dark in [true, false] {
        for scale in oiko_ui::scale::UI_SCALE_STEPS {
            let mut ui = Harness::new(dark, scale);
            let params = ui.editor.params.clone();
            ui.editor.display.store_tempo(143.0);
            unsafe {
                params.rate._internal_set_plain_value(0.83);
                params.flutter_rate._internal_set_plain_value(19.7);
            }
            ui.frame(vec![]);
            ui.frame(vec![]);
            let expected_wow =
                crate::RateDivision::closest_to_hz(0.83, 143.0, crate::rate_sync::WOW);
            let expected_flutter =
                crate::RateDivision::closest_to_hz(19.7, 143.0, crate::rate_sync::FLUTTER);
            ui.click(Id::new(("rate-sync", params.rate_sync.as_ptr())));
            assert!(params.rate_sync.value());
            assert!(!params.flutter_rate_sync.value());
            assert_eq!(params.rate_division.value(), expected_wow);
            assert_eq!(params.flutter_rate.value(), 19.7);
            ui.click(Id::new(("rate-sync", params.flutter_rate_sync.as_ptr())));
            assert!(params.rate_sync.value() && params.flutter_rate_sync.value());
            assert_eq!(params.flutter_rate_division.value(), expected_flutter);
            ui.click(Id::new(("rate-free", params.rate_sync.as_ptr())));
            assert!(!params.rate_sync.value());
            assert!(params.flutter_rate_sync.value());
            assert!((params.rate.value() - expected_wow.rate_hz(143.0)).abs() < 1e-5);
            ui.click(Id::new(("rate-free", params.flutter_rate_sync.as_ptr())));
            assert!(!params.flutter_rate_sync.value());
            assert!((params.flutter_rate.value() - expected_flutter.rate_hz(143.0)).abs() < 1e-5);
        }
    }
}

#[test]
fn synced_drag_uses_hz_positions_instead_of_even_enum_spacing() {
    let mut ui = Harness::new(true, 1.0);
    let params = ui.editor.params.clone();
    unsafe {
        params.flutter_rate_sync._internal_set_plain_value(true);
        params
            .flutter_rate_division
            ._internal_set_plain_value(crate::RateDivision::SixteenthTriplet);
    }
    ui.frame(vec![]);
    ui.frame(vec![]);
    let start = ui
        .context
        .read_response(Id::new(("knob", params.flutter_rate_division.as_ptr())))
        .unwrap()
        .rect
        .center();
    let initial = params.flutter_rate.preview_normalized(12.0);
    let movement = 35.0;
    let expected_hz = params
        .flutter_rate
        .preview_plain(initial + movement * 0.0045);
    let expected =
        crate::RateDivision::closest_to_hz(expected_hz, 120.0, crate::rate_sync::FLUTTER);
    ui.frame(vec![
        egui::Event::PointerMoved(start),
        egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    let end = start - Vec2::new(0.0, movement);
    ui.frame(vec![egui::Event::PointerMoved(end)]);
    ui.frame(vec![egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(params.flutter_rate_division.value(), expected);
    assert_ne!(expected, crate::RateDivision::SixteenthTriplet);
}
