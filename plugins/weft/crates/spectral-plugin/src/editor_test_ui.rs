use super::rendering::frequency_to_x;
use super::*;
use nice_plug::context::{PluginApi, gui::GuiContextInner};
use nice_plug::params::internals::ParamPtr;
use nice_plug::prelude::PluginState;
use oiko_dsp::note_frequency;
use std::collections::HashMap;

struct TestHost;
#[test]
fn mts_ruler_selects_tuned_keys_and_restores_keyboard_without_losing_notes() {
    for dark in [true, false] {
        let mut ui = Harness::new(dark);
        let tuning = crate::mts_client::Tuning {
            active: true,
            frequencies: std::array::from_fn(|n| 440.0 * 2.0_f32.powf((n as f32 - 69.0) / 19.0)),
            map_size: Some(19),
            map_start: 69,
            ..Default::default()
        };
        ui.editor.test_tuning = Some(tuning);
        ui.frame(vec![]);
        ui.frame(vec![]);
        let y = ui
            .context
            .read_response(Id::new("reset-pinned-notes"))
            .unwrap()
            .rect
            .center()
            .y;
        let rect = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(750.0, 36.0));
        let note = 76;
        ui.click(Pos2::new(
            frequency_to_x(tuning.frequencies[note], rect, 22_000.0),
            y,
        ));
        assert!(ui.editor.params.pinned_notes.get(note));
        assert_eq!(
            (0..128)
                .filter(|&n| ui.editor.params.pinned_notes.get(n))
                .count(),
            1
        );
        let output = ui.frame(vec![]);
        ui.screenshot(
            output,
            if dark {
                "mts-19edo-dark"
            } else {
                "mts-19edo-bright"
            },
        );
        ui.shortcut(false);
        assert!(!ui.editor.params.pinned_notes.get(note));
        ui.shortcut(true);
        assert!(ui.editor.params.pinned_notes.get(note));
        ui.editor.test_tuning = Some(crate::mts_client::Tuning::default());
        ui.frame(vec![]);
        assert!(ui.editor.params.pinned_notes.get(note));
    }
}
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
    editor: SpectralEditor,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
}

impl Harness {
    fn new(dark: bool) -> Self {
        let context = egui::Context::default();
        apply_theme(&context, dark);
        context.set_pixels_per_point(1.25);
        let params = Arc::new(SpectralParams::default());
        let display = Arc::new(AnalysisDisplay::default());
        let mut editor = SpectralEditor::new(params, display);
        editor.dark.store(dark, Ordering::Relaxed);
        editor.gui_context = Some(GuiContext::new(Arc::new(TestHost)));
        editor.ruler_intro_started = Instant::now() - Duration::from_secs(1);
        Self {
            context,
            editor,
            textures: HashMap::new(),
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        // Test gestures use content coordinates. macOS scales that content layer
        // inside the viewport, so native pointer events must use viewport coordinates.
        #[cfg(target_os = "macos")]
        let content_scale = closest_ui_scale(self.editor.params.ui_scale.get());
        #[cfg(not(target_os = "macos"))]
        let content_scale = 1.0;
        let events = events
            .into_iter()
            .map(|event| match event {
                egui::Event::PointerMoved(pos) => egui::Event::PointerMoved(pos * content_scale),
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => egui::Event::PointerButton {
                    pos: pos * content_scale,
                    button,
                    pressed,
                    modifiers,
                },
                event => event,
            })
            .collect();
        let viewport_size = Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT) * content_scale;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, viewport_size)),
            events,
            ..Default::default()
        };
        self.context.begin_pass(input);
        let mut root = egui::Ui::new(
            self.context.clone(),
            Id::new("test-root"),
            UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, viewport_size)),
        );
        self.editor.draw_ui(&mut root);
        let mut output = self.context.end_pass();
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                if let Some([x, y]) = delta.pos {
                    let atlas = self.textures.get_mut(id).unwrap();
                    for row in 0..image.height() {
                        for col in 0..image.width() {
                            atlas[(x + col, y + row)] = image[(col, row)];
                        }
                    }
                } else {
                    self.textures.insert(*id, image.as_ref().clone());
                }
            }
        }
        output.textures_delta.clear();
        output
    }

    fn click(&mut self, pos: Pos2) {
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

    fn shortcut(&mut self, shift: bool) {
        self.frame(vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ctrl: true,
                shift,
                ..Default::default()
            },
        }]);
        self.frame(vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }

    fn parameter_rect(&self, name: &str) -> Rect {
        self.context
            .read_response(Id::new(("compact-numeric", name)))
            .unwrap_or_else(|| panic!("Missing parameter field: {name}"))
            .rect
    }

    fn note_position(&self, note: u8) -> Pos2 {
        let y = self
            .context
            .read_response(Id::new("hold-incoming-notes"))
            .unwrap()
            .rect
            .center()
            .y;
        let rect = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(750.0, 36.0));
        Pos2::new(frequency_to_x(note_frequency(note), rect, 22_000.0), y)
    }

    // Rasterize egui's actual meshes and font atlas for optional visual QA.
    fn screenshot(&self, output: egui::FullOutput, name: &str) {
        let Ok(directory) = std::env::var("WEFT_UI_QA_DIR") else {
            return;
        };
        let scale = output.pixels_per_point;
        let width = (EDITOR_WIDTH * scale).round() as usize;
        let height = (EDITOR_HEIGHT * scale).round() as usize;
        let mut pixels = vec![[0.0_f32; 3]; width * height];
        for primitive in self.context.tessellate(output.shapes, scale) {
            let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                continue;
            };
            let texture = &self.textures[&mesh.texture_id];
            let clip = primitive.clip_rect * scale;
            for triangle in mesh.indices.as_chunks::<3>().0 {
                let vertices = [
                    mesh.vertices[triangle[0] as usize],
                    mesh.vertices[triangle[1] as usize],
                    mesh.vertices[triangle[2] as usize],
                ];
                let [a, b, c] = vertices.map(|v| v.pos * scale);
                let area = cross(b - a, c - a);
                if area.abs() < 1e-6 {
                    continue;
                }
                let left = a.x.min(b.x).min(c.x).max(clip.left()).max(0.0).floor() as usize;
                let right =
                    a.x.max(b.x)
                        .max(c.x)
                        .min(clip.right())
                        .min(width as f32)
                        .ceil() as usize;
                let top = a.y.min(b.y).min(c.y).max(clip.top()).max(0.0).floor() as usize;
                let bottom =
                    a.y.max(b.y)
                        .max(c.y)
                        .min(clip.bottom())
                        .min(height as f32)
                        .ceil() as usize;
                for y in top..bottom {
                    for x in left..right {
                        let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                        let weights = [
                            cross(b - p, c - p) / area,
                            cross(c - p, a - p) / area,
                            cross(a - p, b - p) / area,
                        ];
                        if weights.iter().any(|w| *w < 0.0) {
                            continue;
                        }
                        let uv = vertices
                            .iter()
                            .zip(weights)
                            .fold(Vec2::ZERO, |sum, (v, w)| sum + v.uv.to_vec2() * w);
                        let tx =
                            ((uv.x * texture.width() as f32) as usize).min(texture.width() - 1);
                        let ty =
                            ((uv.y * texture.height() as f32) as usize).min(texture.height() - 1);
                        let texel = texture[(tx, ty)].to_array();
                        let mut color = [0.0; 4];
                        for (v, w) in vertices.iter().zip(weights) {
                            for (channel, value) in color.iter_mut().zip(v.color.to_array()) {
                                *channel += value as f32 / 255.0 * w;
                            }
                        }
                        for (channel, texel) in color.iter_mut().zip(texel) {
                            *channel *= texel as f32 / 255.0;
                        }
                        for (target, source) in pixels[y * width + x].iter_mut().zip(color) {
                            *target = source + *target * (1.0 - color[3]);
                        }
                    }
                }
            }
        }
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        bytes.extend(
            pixels
                .into_iter()
                .flatten()
                .map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8),
        );
        std::fs::write(
            std::path::Path::new(&directory).join(format!("{name}.ppm")),
            bytes,
        )
        .unwrap();
    }
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

#[test]
fn ordinary_parameter_edit_can_be_undone_with_pointer_still_inside() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let before = ui.editor.params.motion_shape.value();
    let next = ui
        .context
        .read_response(Id::new(("step-value-next", "SHAPE")))
        .unwrap()
        .rect
        .center();
    ui.click(next);
    let after = ui.editor.params.motion_shape.value();
    assert_ne!(before, after);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.motion_shape.value(), before);
    ui.shortcut(true);
    assert_eq!(ui.editor.params.motion_shape.value(), after);
    ui.shortcut(true);
    assert_eq!(ui.editor.params.motion_shape.value(), after);
}

#[test]
fn sync_mode_and_converted_rate_are_one_undo_step() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let before_sync = ui.editor.params.motion_sync.value();
    let before_division = ui.editor.params.motion_rate_division.value();
    let choice = if before_sync {
        "motion-rate-free-choice"
    } else {
        "motion-rate-sync-choice"
    };
    let pos = ui
        .context
        .read_response(Id::new(choice))
        .unwrap()
        .rect
        .center();
    ui.click(pos);
    assert_ne!(ui.editor.params.motion_sync.value(), before_sync);
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.motion_sync.value(), before_sync);
    assert_eq!(
        ui.editor.params.motion_rate_division.value(),
        before_division
    );
}

#[test]
fn a_continuous_parameter_drag_is_one_undo_step() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let rect = ui.parameter_rect(ui.editor.params.partials.name());
    let point = |fraction| Pos2::new(rect.left() + rect.width() * fraction, rect.center().y);
    let before = ui.editor.params.partials.value();
    ui.frame(vec![egui::Event::PointerMoved(point(0.1))]);
    ui.frame(vec![egui::Event::PointerButton {
        pos: point(0.1),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    for fraction in [0.2, 0.35, 0.5, 0.65, 0.8] {
        ui.frame(vec![egui::Event::PointerMoved(point(fraction))]);
    }
    ui.frame(vec![egui::Event::PointerButton {
        pos: point(0.8),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    let after = ui.editor.params.partials.value();
    assert_ne!(after, before);
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.partials.value(), before);
    ui.shortcut(true);
    assert_eq!(ui.editor.params.partials.value(), after);
}

#[test]
fn parameters_notes_and_curves_undo_in_the_order_they_were_edited() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let before_shape = ui.editor.params.motion_shape.value();
    let mut before_curve = [0.0; MANUAL_MASK_POINTS];
    ui.editor.params.curve.copy_to(&mut before_curve);
    let shape_next = ui
        .context
        .read_response(Id::new(("step-value-next", "SHAPE")))
        .unwrap()
        .rect
        .center();
    ui.click(shape_next);
    let after_shape = ui.editor.params.motion_shape.value();
    ui.click(ui.note_position(60));
    ui.click(Pos2::new(400.0, 170.0));
    let mut after_curve = [0.0; MANUAL_MASK_POINTS];
    ui.editor.params.curve.copy_to(&mut after_curve);
    assert_ne!(after_curve, before_curve);
    assert_eq!(ui.editor.curve_history.undo.len(), 3);

    ui.shortcut(false);
    let mut actual_curve = [0.0; MANUAL_MASK_POINTS];
    ui.editor.params.curve.copy_to(&mut actual_curve);
    assert_eq!(actual_curve, before_curve);
    assert!(ui.editor.params.pinned_notes.get(60));
    assert_eq!(ui.editor.params.motion_shape.value(), after_shape);
    ui.shortcut(false);
    assert!(!ui.editor.params.pinned_notes.get(60));
    assert_eq!(ui.editor.params.motion_shape.value(), after_shape);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.motion_shape.value(), before_shape);

    ui.shortcut(true);
    assert_eq!(ui.editor.params.motion_shape.value(), after_shape);
    assert!(!ui.editor.params.pinned_notes.get(60));
    ui.shortcut(true);
    assert!(ui.editor.params.pinned_notes.get(60));
    ui.editor.params.curve.copy_to(&mut actual_curve);
    assert_eq!(actual_curve, before_curve);
    ui.shortcut(true);
    ui.editor.params.curve.copy_to(&mut actual_curve);
    assert_eq!(actual_curve, after_curve);
}

#[test]
fn host_changes_stay_out_of_history_and_survive_unrelated_undo() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let before = ui.editor.params.partials.value();
    ui.click(ui.parameter_rect(ui.editor.params.partials.name()).center());
    assert_ne!(ui.editor.params.partials.value(), before);
    assert_eq!(ui.editor.curve_history.undo.len(), 1);

    // Simulate host automation without passing through a GUI gesture. The
    // Harness keeps the parameter's owning Arc alive throughout these calls.
    let output = &ui.editor.params.output_gain_db;
    unsafe {
        output
            .as_ptr()
            ._internal_set_normalized_value(output.preview_normalized(-7.0));
    }
    ui.frame(vec![]);
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.partials.value(), before);
    assert!((ui.editor.params.output_gain_db.value() + 7.0).abs() < 0.001);
    assert_eq!(ui.editor.curve_history.redo.len(), 1);

    let output = &ui.editor.params.output_gain_db;
    unsafe {
        output
            .as_ptr()
            ._internal_set_normalized_value(output.preview_normalized(-11.0));
    }
    ui.frame(vec![]);
    assert!(ui.editor.curve_history.undo.is_empty());
    assert_eq!(ui.editor.curve_history.redo.len(), 1);
    ui.shortcut(true);
    assert_ne!(ui.editor.params.partials.value(), before);
    assert!((ui.editor.params.output_gain_db.value() + 11.0).abs() < 0.001);
}

#[test]
fn a_no_op_parameter_click_preserves_redo() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let shape_next = ui
        .context
        .read_response(Id::new(("step-value-next", "SHAPE")))
        .unwrap()
        .rect
        .center();
    ui.click(shape_next);
    let after = ui.editor.params.motion_shape.value();
    ui.shortcut(false);
    assert_eq!(ui.editor.curve_history.redo.len(), 1);
    let rect = ui.parameter_rect(ui.editor.params.partials.name());
    // The first pixel still snaps to the minimum integer value, one partial.
    ui.click(Pos2::new(rect.left() + 1.0, rect.center().y));
    assert_eq!(ui.editor.params.partials.value(), 1);
    assert!(ui.editor.curve_history.undo.is_empty());
    assert_eq!(ui.editor.curve_history.redo.len(), 1);
    ui.shortcut(true);
    assert_eq!(ui.editor.params.motion_shape.value(), after);
}

#[test]
fn numeric_text_undo_does_not_pop_the_sound_history() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    ui.click(ui.parameter_rect(ui.editor.params.partials.name()).center());
    let after = ui.editor.params.partials.value();
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    let edit = Id::new(("compact-numeric", ui.editor.params.partials.name())).with("edit");
    ui.context.memory_mut(|memory| memory.request_focus(edit));
    ui.frame(vec![]);
    assert!(ui.context.text_edit_focused());
    ui.shortcut(false);
    assert_eq!(ui.editor.params.partials.value(), after);
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    assert!(ui.editor.curve_history.redo.is_empty());

    ui.context.memory_mut(|memory| memory.surrender_focus(edit));
    ui.frame(vec![]);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.partials.value(), 1);
}

#[test]
fn bent_arrow_buttons_undo_and_redo_ordinary_parameters() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let before = ui.editor.params.partials.value();
    ui.click(ui.parameter_rect(ui.editor.params.partials.name()).center());
    let after = ui.editor.params.partials.value();
    assert_ne!(before, after);
    let undo = ui
        .context
        .read_response(Id::new("undo-curve"))
        .unwrap()
        .rect
        .center();
    let redo = ui
        .context
        .read_response(Id::new("redo-curve"))
        .unwrap()
        .rect
        .center();
    ui.click(undo);
    assert_eq!(ui.editor.params.partials.value(), before);
    ui.click(redo);
    assert_eq!(ui.editor.params.partials.value(), after);
    ui.click(redo);
    assert_eq!(ui.editor.params.partials.value(), after);
}

#[test]
fn footer_resolution_change_is_undoable_despite_not_being_automatable() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let before = ui.editor.params.quality.value();
    let previous = ui
        .context
        .read_response(Id::new(("footer-param-prev", "RESOLUTION")))
        .unwrap()
        .rect
        .center();
    ui.click(previous);
    let after = ui.editor.params.quality.value();
    assert_ne!(after, before);
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.quality.value(), before);
    ui.shortcut(true);
    assert_eq!(ui.editor.params.quality.value(), after);
}

#[test]
fn undo_during_a_parameter_drag_waits_until_the_gesture_is_finished() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    ui.click(ui.note_position(60));
    let before = ui.editor.params.partials.value();
    let rect = ui.parameter_rect(ui.editor.params.partials.name());
    let start = Pos2::new(rect.left() + 2.0, rect.center().y);
    let end = rect.center();
    ui.frame(vec![egui::Event::PointerMoved(start)]);
    ui.frame(vec![egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    ui.frame(vec![egui::Event::PointerMoved(end)]);
    let after = ui.editor.params.partials.value();
    assert_ne!(before, after);
    assert!(ui.editor.parameter_gestures.is_active());

    ui.shortcut(false);
    assert_eq!(ui.editor.params.partials.value(), after);
    assert!(ui.editor.params.pinned_notes.get(60));
    assert!(ui.editor.parameter_gestures.is_active());
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    assert!(ui.editor.curve_history.redo.is_empty());

    ui.frame(vec![egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(!ui.editor.parameter_gestures.is_active());
    assert_eq!(ui.editor.curve_history.undo.len(), 2);
    ui.shortcut(false);
    assert_eq!(ui.editor.params.partials.value(), before);
    assert!(ui.editor.params.pinned_notes.get(60));
    ui.shortcut(false);
    assert!(!ui.editor.params.pinned_notes.get(60));
}

#[test]
fn a_drag_finishes_when_host_automation_hides_its_control() {
    let mut ui = Harness::new(true);
    // Arrange a host-provided free-rate starting state without a local edit.
    unsafe {
        ui.editor
            .params
            .motion_sync
            .as_ptr()
            ._internal_set_normalized_value(0.0);
    }
    ui.frame(vec![]);
    ui.frame(vec![]);
    assert!(!ui.editor.params.motion_sync.value());
    let before = ui.editor.params.motion_rate_hz.value();
    let rect = ui.parameter_rect(ui.editor.params.motion_rate_hz.name());
    let start = Pos2::new(rect.left() + 2.0, rect.center().y);
    let end = rect.center();
    ui.frame(vec![egui::Event::PointerMoved(start)]);
    ui.frame(vec![egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    ui.frame(vec![egui::Event::PointerMoved(end)]);
    assert!(ui.editor.parameter_gestures.is_active());
    assert_ne!(ui.editor.params.motion_rate_hz.value(), before);

    // Host automation replaces the free-rate field with the synced-rate field
    // before mouse-up, so the original widget cannot report drag_stopped.
    // Its owning Arc remains alive for this simulated host update.
    unsafe {
        ui.editor
            .params
            .motion_sync
            .as_ptr()
            ._internal_set_normalized_value(1.0);
    }
    ui.frame(vec![]);
    ui.frame(vec![egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(!ui.editor.parameter_gestures.is_active());
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    ui.shortcut(false);
    assert!((ui.editor.params.motion_rate_hz.value() - before).abs() < 0.0001);
    assert!(ui.editor.params.motion_sync.value());
}

#[test]
fn pinned_note_click_and_host_shortcuts_share_one_local_history() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let hold_rect = ui
        .context
        .read_response(Id::new("hold-incoming-notes"))
        .unwrap()
        .rect;
    let key_rect = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(750.0, 36.0));
    let x = frequency_to_x(note_frequency(60), key_rect, 22_000.0);
    ui.click(Pos2::new(x, hold_rect.center().y));
    assert!(ui.editor.params.pinned_notes.get(60));
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    ui.shortcut(false);
    assert!(!ui.editor.params.pinned_notes.get(60));
    ui.shortcut(true);
    assert!(ui.editor.params.pinned_notes.get(60));
    ui.shortcut(true);
    assert!(ui.editor.params.pinned_notes.get(60));
}

#[test]
fn capture_preview_and_transfer_menu_render_in_both_themes() {
    for dark in [true, false] {
        let mut ui = Harness::new(dark);
        ui.editor.params.pinned_notes.set(60, true);
        ui.editor.display.note_levels[60].store(0.7_f32.to_bits(), Ordering::Relaxed);
        ui.editor.display.store_output_peak(1.2);
        let levels = std::array::from_fn(|i| -12.0 - 30.0 * ((i as f32 / 20.0).sin()).abs());
        ui.editor.display.store_spectrum(&levels);
        ui.editor.smoothed_spectrum_db = levels;
        ui.editor.capture_active = true;
        ui.editor.capture_epoch = ui.editor.display.capture.begin();
        let mut accumulator = crate::capture::CaptureAccumulator::default();
        accumulator.push(&levels, 0.1, &ui.editor.display.capture);
        ui.frame(vec![]);
        let output = ui.frame(vec![]);
        assert!(!output.shapes.is_empty());
        ui.screenshot(
            output,
            if dark {
                "capture-dark"
            } else {
                "capture-light"
            },
        );
        let menu = ui
            .context
            .read_response(Id::new("curve-transfer"))
            .unwrap()
            .rect;
        ui.click(menu.center());
        for _ in 0..15 {
            ui.frame(vec![]);
        }
        let output = ui.frame(vec![]);
        ui.screenshot(
            output,
            if dark {
                "curve-menu-dark"
            } else {
                "curve-menu-light"
            },
        );
        ui.click(menu.center());
        ui.editor.capture_active = false;
        ui.editor.display.capture.stop();
        let setter = ui.editor.gui_context.as_ref().unwrap().param_setter();
        setter.set_parameter(&ui.editor.params.partials, 8);
        setter.set_parameter(&ui.editor.params.note_depth_db, 24.0);
        ui.editor.displayed_note_depth_db = 24.0;
        let y = ui
            .context
            .read_response(Id::new("hold-incoming-notes"))
            .unwrap()
            .rect
            .center()
            .y;
        let key_rect = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(750.0, 36.0));
        let x = frequency_to_x(note_frequency(60), key_rect, 22_000.0);
        ui.frame(vec![egui::Event::PointerMoved(Pos2::new(x, y))]);
        let output = ui.frame(vec![]);
        ui.screenshot(
            output,
            if dark {
                "harmonics-dark"
            } else {
                "harmonics-light"
            },
        );
    }
}

#[test]
fn hold_and_reset_undo_preserve_the_two_kinds_of_pinned_notes() {
    let mut ui = Harness::new(true);
    ui.editor.params.pinned_notes.set(60, true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let hold = ui
        .context
        .read_response(Id::new("hold-incoming-notes"))
        .unwrap()
        .rect
        .center();
    let reset = ui
        .context
        .read_response(Id::new("reset-pinned-notes"))
        .unwrap()
        .rect
        .center();
    ui.click(hold);
    ui.editor.params.pinned_notes.capture(64);
    ui.click(reset);
    assert!(!ui.editor.params.pinned_notes.get(60));
    assert!(!ui.editor.params.pinned_notes.get(64));
    ui.shortcut(false);
    assert!(ui.editor.params.pinned_notes.get(60));
    assert!(ui.editor.params.pinned_notes.get(64));
    ui.click(hold);
    assert!(ui.editor.params.pinned_notes.get(60));
    assert!(!ui.editor.params.pinned_notes.get(64));
    ui.shortcut(false);
    assert!(ui.editor.params.pinned_notes.get(64));
}

#[test]
fn a_drag_across_keys_is_one_undo_step() {
    let mut ui = Harness::new(true);
    ui.frame(vec![]);
    ui.frame(vec![]);
    let y = ui
        .context
        .read_response(Id::new("hold-incoming-notes"))
        .unwrap()
        .rect
        .center()
        .y;
    let rect = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(750.0, 36.0));
    let pos = |note| Pos2::new(frequency_to_x(note_frequency(note), rect, 22_000.0), y);
    ui.frame(vec![egui::Event::PointerMoved(pos(60))]);
    ui.frame(vec![egui::Event::PointerButton {
        pos: pos(60),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    for note in 60..=64 {
        ui.frame(vec![egui::Event::PointerMoved(pos(note))]);
    }
    ui.frame(vec![egui::Event::PointerButton {
        pos: pos(64),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(ui.editor.curve_history.undo.len(), 1);
    assert!(ui.editor.params.pinned_notes.get(64));
    ui.shortcut(false);
    assert!(!(60..=64).any(|note| ui.editor.params.pinned_notes.get(note)));
}
