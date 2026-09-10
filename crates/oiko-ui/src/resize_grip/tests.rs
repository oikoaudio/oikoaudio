use super::*;
use egui::{Context, Event, Modifiers, PointerButton, RawInput};

struct Harness {
    context: Context,
    canvas: Vec2,
    scale: f32,
    dpi: f32,
    focused: bool,
    changes: Vec<f32>,
}

impl Harness {
    fn new(scale: f32, dpi: f32) -> Self {
        let context = Context::default();
        crate::scale::initialize_scale(&context, scale, Vec2::new(760.0, 560.0));
        let mut harness = Self {
            context,
            canvas: Vec2::new(760.0, 560.0),
            scale,
            dpi,
            focused: true,
            changes: Vec::new(),
        };
        harness.frame(vec![]);
        harness.frame(vec![]);
        harness
    }

    fn frame(&mut self, events: Vec<Event>) {
        let viewport = self.canvas * crate::scale::canvas_scale(self.scale);
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, viewport)),
            events,
            focused: self.focused,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.dpi);
        let mut output = self.context.run_ui(input, |ui| {
            if let Some(scale) = show(ui, self.canvas, self.scale) {
                self.changes.push(scale);
                self.scale = scale;
                crate::scale::request_scale(ui.ctx(), scale, self.canvas);
            }
        });
        output.textures_delta.clear();
    }

    fn press(&mut self) -> Pos2 {
        let corner = self
            .context
            .read_response(Id::new("oiko-resize-grip"))
            .unwrap()
            .rect
            .center();
        self.frame(vec![Event::PointerMoved(corner)]);
        self.frame(vec![button(corner, true)]);
        corner
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn resize_is_committed_once_on_release_and_does_not_bounce_after_window_changes() {
    for dpi in [1.0, 2.0] {
        for (start, target) in [(1.0, 1.25), (1.25, 1.0), (0.5, 2.0), (2.0, 0.5)] {
            let mut ui = Harness::new(start, dpi);
            let origin = ui.press();
            let position = origin + ui.canvas * (target - start) / ui.context.zoom_factor();
            ui.frame(vec![Event::PointerMoved(position)]);
            for _ in 0..4 {
                ui.frame(vec![]);
                assert_eq!(ui.scale, start);
                assert!(ui.changes.is_empty());
            }
            // A growing drag releases outside the old window. Capture must persist.
            ui.frame(vec![button(position, false)]);
            assert_eq!(ui.changes, [target]);
            // Simulate the host accepting the new size, followed by stationary frames.
            for _ in 0..4 {
                ui.frame(vec![Event::PointerMoved(position)]);
            }
            assert_eq!(ui.scale, target);
            assert_eq!(ui.changes, [target]);
        }
    }
}

#[test]
fn escape_cancels_without_resizing_and_a_later_drag_still_works() {
    let mut ui = Harness::new(1.0, 2.0);
    let origin = ui.press();
    let position = origin + ui.canvas * 0.25;
    ui.frame(vec![Event::PointerMoved(position)]);
    ui.frame(vec![Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    ui.frame(vec![button(position, false)]);
    assert!(ui.changes.is_empty());
    let origin = ui.press();
    let position = origin + ui.canvas * 0.25;
    ui.frame(vec![Event::PointerMoved(position)]);
    ui.frame(vec![button(position, false)]);
    assert_eq!(ui.changes, [1.25]);
}

#[test]
fn preview_has_hysteresis_and_clamps_to_supported_scales() {
    let canvas = Vec2::new(640.0, 400.0);
    let mut drag = Drag {
        origin: Pos2::new(630.0, 390.0),
        scale: 1.0,
        target: 1.0,
        canvas,
    };
    for (movement, target) in [
        (0.14, 1.0),
        (0.16, 1.25),
        (0.12, 1.25),
        (0.10, 1.0),
        (9.0, 2.0),
        (-9.0, 0.5),
    ] {
        drag.update(drag.origin + canvas * movement);
        assert_eq!(drag.target, target);
    }
}

#[test]
fn losing_focus_or_changing_zoom_cancels_the_pending_drag() {
    for lose_focus in [true, false] {
        let mut ui = Harness::new(1.0, 1.0);
        let origin = ui.press();
        let position = origin + ui.canvas * 0.25;
        ui.frame(vec![Event::PointerMoved(position)]);
        if lose_focus {
            ui.focused = false;
        } else {
            ui.scale = 1.5;
        }
        ui.frame(vec![]);
        ui.frame(vec![button(position, false)]);
        assert!(ui.changes.is_empty());
    }
}

#[test]
fn clicking_or_returning_to_the_original_size_does_not_resize() {
    for move_away in [true, false] {
        let mut ui = Harness::new(1.0, 1.0);
        let origin = ui.press();
        if move_away {
            ui.frame(vec![Event::PointerMoved(origin + ui.canvas * 0.25)]);
            ui.frame(vec![Event::PointerMoved(origin)]);
        }
        ui.frame(vec![button(origin, false)]);
        assert!(ui.changes.is_empty());
    }
}
