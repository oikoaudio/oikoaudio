use super::*;
use nice_plug::context::{PluginApi, gui::GuiContextInner};
use nice_plug::params::{InternalParamMut, internals::ParamPtr};
use nice_plug::prelude::PluginState;

#[derive(Default)]
struct TestHost {
    gestures: std::sync::Mutex<Vec<(bool, ParamPtr)>>,
}
impl GuiContextInner for TestHost {
    fn request_restart(&self) {}

    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    unsafe fn raw_begin_set_parameter(&self, param: ParamPtr) {
        self.gestures.lock().unwrap().push((true, param));
    }
    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, value: f32) {
        unsafe {
            param._internal_set_normalized_value(value);
        }
    }
    unsafe fn raw_end_set_parameter(&self, param: ParamPtr) {
        self.gestures.lock().unwrap().push((false, param));
    }
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
    host: Arc<TestHost>,
}
impl Harness {
    fn new(dark: bool, scale: f32) -> Self {
        let context = egui::Context::default();
        apply_theme(&context, dark);
        context.set_pixels_per_point(scale);
        let params = Arc::new(WowParams::default());
        let mut editor = WowEditor::new(params, Arc::new(ModulationDisplay::default()));
        editor.dark.store(dark, Ordering::Relaxed);
        let host = Arc::new(TestHost::default());
        editor.gui_context = Some(GuiContext::new(host.clone()));
        Self {
            context,
            editor,
            host,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.frame_with_modifiers(events, egui::Modifiers::NONE)
    }
    fn frame_with_modifiers(
        &mut self,
        mut events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::FullOutput {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
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

impl Harness {
    fn drag(&mut self, id: Id, movement: Vec2) {
        let start = self.context.read_response(id).unwrap().rect.center();
        self.frame(vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let end = start + movement;
        self.frame(vec![egui::Event::PointerMoved(end)]);
        self.frame(vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}

#[test]
fn phase_wraps_and_spread_and_balance_edit_their_own_parameters() {
    for dark in [true, false] {
        for scale in oiko_ui::scale::UI_SCALE_STEPS {
            let mut ui = Harness::new(dark, scale);
            let params = ui.editor.params.clone();
            unsafe {
                params.phase_offset._internal_set_plain_value(359.0 / 360.0);
            }
            ui.frame(vec![]);
            ui.frame(vec![]);
            ui.drag(
                Id::new(("knob", params.phase_offset.as_ptr())),
                Vec2::new(0.0, -20.0),
            );
            let phase = params.phase_offset.value();
            assert!((phase - (359.0_f32 / 360.0 + 0.09).rem_euclid(1.0)).abs() < 1e-5);
            assert_eq!(params.stereo.value(), 0.0);
            ui.drag(
                Id::new(("spread-value", params.stereo.as_ptr())),
                Vec2::new(20.0, 0.0),
            );
            assert!((params.stereo.value() - 0.09).abs() < 1e-5);
            assert_eq!(params.phase_offset.value(), phase);
            let id = Id::new(("slider", params.wow_flutter.as_ptr()));
            let rect = ui.context.read_response(id).unwrap().rect;
            // Drag to the right endpoint: full Wow means the existing Flutter fraction is zero.
            ui.drag(id, Vec2::new(rect.width() * 0.5 - 8.0, 0.0));
            assert!(params.wow_flutter.value() < 1e-5);
            ui.drag(id, Vec2::new(-rect.width() * 0.5 + 8.0, 0.0));
            assert!((params.wow_flutter.value() - 1.0).abs() < 1e-5);
            assert_eq!(params.phase_offset.value(), phase);
        }
    }
}

#[test]
fn phase_dial_alt_drag_keeps_its_parameter_and_host_gesture_until_release() {
    for alt in [false, true] {
        for shift in [false, true] {
            for movement in [40.0, 400.0] {
                let mut ui = Harness::new(true, 1.0);
                let params = ui.editor.params.clone();
                unsafe {
                    params.phase_offset._internal_set_plain_value(0.25);
                    params.stereo._internal_set_plain_value(0.5);
                }
                ui.frame(vec![]);
                ui.frame(vec![]);
                let start = ui
                    .context
                    .read_response(Id::new(("knob", params.phase_offset.as_ptr())))
                    .unwrap()
                    .rect
                    .center();
                let modifiers = egui::Modifiers {
                    alt,
                    shift,
                    ..Default::default()
                };
                ui.frame_with_modifiers(
                    vec![
                        egui::Event::PointerMoved(start),
                        egui::Event::PointerButton {
                            pos: start,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers,
                        },
                    ],
                    modifiers,
                );
                // Changing Alt before the drag threshold must not change the target.
                let modifiers = egui::Modifiers {
                    alt: !alt,
                    ..modifiers
                };
                for distance in [20.0, movement] {
                    ui.frame_with_modifiers(
                        vec![egui::Event::PointerMoved(start - Vec2::new(0.0, distance))],
                        modifiers,
                    );
                }
                ui.frame_with_modifiers(
                    vec![egui::Event::PointerButton {
                        pos: start - Vec2::new(0.0, movement),
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers,
                    }],
                    modifiers,
                );
                let delta = movement * 0.0045 * if shift { 0.2 } else { 1.0 };
                let target = if alt {
                    assert_eq!(params.phase_offset.value(), 0.25);
                    assert!((params.stereo.value() - (0.5 + delta).clamp(0.0, 1.0)).abs() < 1e-5);
                    params.stereo.as_ptr()
                } else {
                    assert_eq!(params.stereo.value(), 0.5);
                    assert!(
                        (params.phase_offset.value() - (0.25 + delta).rem_euclid(1.0)).abs() < 1e-5
                    );
                    params.phase_offset.as_ptr()
                };
                assert_eq!(
                    *ui.host.gestures.lock().unwrap(),
                    [(true, target), (false, target)]
                );
            }
        }
    }
}
