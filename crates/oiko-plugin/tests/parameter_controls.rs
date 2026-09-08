use egui::{Event, Id, Modifiers, PointerButton, Pos2, RawInput, Rect, Sense, Vec2};
use nice_plug::prelude::{FloatParam, FloatRange, Param};
use oiko_plugin::{
    gestures::ParameterWriter,
    parameter_controls::{DragMode, parameter_drag},
};
use std::cell::RefCell;

#[derive(Default)]
struct Writer(RefCell<Vec<Option<f32>>>);
impl ParameterWriter for Writer {
    fn begin_set_parameter<P: Param>(&self, _: &P) {
        self.0.borrow_mut().push(None);
    }
    fn set_parameter<P: Param>(&self, param: &P, value: P::Plain) {
        self.0
            .borrow_mut()
            .push(Some(param.preview_normalized(value)));
    }
    fn end_set_parameter<P: Param>(&self, _: &P) {
        self.0.borrow_mut().push(None);
    }
}

#[test]
fn drag_modes_preserve_sensitivity_and_one_gesture_without_host_echo() {
    let range = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));
    for (mode, shift, expected) in [
        (
            DragMode::Relative {
                sensitivity: 0.0045,
                horizontal_weight: 0.35,
                fine_scale: 0.2,
            },
            false,
            0.5695,
        ),
        (
            DragMode::Relative {
                sensitivity: 0.0045,
                horizontal_weight: 0.35,
                fine_scale: 0.2,
            },
            true,
            0.3139,
        ),
        (
            DragMode::Absolute {
                range,
                fine_sensitivity: None,
            },
            false,
            0.8,
        ),
        (
            DragMode::Absolute {
                range,
                fine_sensitivity: Some(0.0015),
            },
            true,
            0.415,
        ),
    ] {
        let ctx = egui::Context::default();
        let writer = Writer::default();
        let param = FloatParam::new("Value", 0.25, FloatRange::Linear { min: 0.0, max: 1.0 });
        let modifiers = Modifiers {
            shift,
            ..Default::default()
        };
        let mut time = 0.0;
        let mut frame = |mut events: Vec<Event>| {
            events.insert(0, Event::ModifiersChanged(modifiers));
            time += 0.02;
            let mut output = ctx.run_ui(
                RawInput {
                    events,
                    time: Some(time),
                    screen_rect: Some(range),
                    ..Default::default()
                },
                |ui| {
                    let response = ui.interact(range, Id::new("control"), Sense::click_and_drag());
                    parameter_drag(ui, &response, &param, &writer, mode);
                },
            );
            output.textures_delta.clear();
        };
        frame(vec![]);
        let start = Pos2::new(20.0, 50.0);
        frame(vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            },
        ]);
        frame(vec![Event::PointerMoved(Pos2::new(50.0, 30.0))]);
        let end = Pos2::new(80.0, 0.0);
        frame(vec![Event::PointerMoved(end)]);
        frame(vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers,
        }]);
        let events = writer.0.borrow();
        assert_eq!(events.iter().filter(|e| e.is_none()).count(), 2);
        assert_eq!(events.first(), Some(&None));
        assert_eq!(events.last(), Some(&None));
        let value = events.iter().filter_map(|v| *v).next_back().unwrap();
        assert!((value - expected).abs() < 1e-6, "{value} != {expected}");
    }
}
