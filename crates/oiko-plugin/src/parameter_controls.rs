//! Parameter interaction only. Products own painting, hit rectangles and shortcuts.
use crate::gestures::ParameterWriter;
use egui::{Rect, Response, Ui};
use nice_plug::params::Param;

#[derive(Clone, Copy)]
pub enum DragMode {
    Relative {
        sensitivity: f32,
        horizontal_weight: f32,
        fine_scale: f32,
    },
    Absolute {
        range: Rect,
        fine_sensitivity: Option<f32>,
    },
}

/// One host gesture across drag frames. Shift behaviour stays explicit at each
/// control; focus loss and editor closure are handled by ParameterGestures.
pub fn parameter_drag<P: Param>(
    ui: &Ui,
    response: &Response,
    param: &P,
    setter: &impl ParameterWriter,
    mode: DragMode,
) {
    parameter_drag_mapped(
        ui,
        response,
        param,
        setter,
        mode,
        (param.unmodulated_normalized_value(), |value| {
            param.preview_plain(value)
        }),
    );
}

/// Use a display-space position and inverse mapping while retaining host gesture semantics.
/// This lets musical divisions occupy their equivalent Hz positions on a rate control.
pub fn parameter_drag_mapped<P: Param>(
    ui: &Ui,
    response: &Response,
    param: &P,
    setter: &impl ParameterWriter,
    mode: DragMode,
    mapping: (f32, impl Fn(f32) -> P::Plain),
) {
    let (normalized, from_normalized) = mapping;
    let memory_id = response.id.with("drag-start");
    if response.drag_started() {
        setter.begin_set_parameter(param);
        ui.data_mut(|data| data.insert_temp(memory_id, normalized));
    }
    if response.dragged() {
        let start = ui
            .data(|data| data.get_temp::<f32>(memory_id))
            .unwrap_or(normalized);
        let delta = response.total_drag_delta().unwrap_or_default();
        let shift = ui.input(|input| input.modifiers.shift);
        let value = match mode {
            DragMode::Relative {
                sensitivity,
                horizontal_weight,
                fine_scale,
            } => {
                let movement = -delta.y + delta.x * horizontal_weight;
                Some(start + movement * sensitivity * if shift { fine_scale } else { 1.0 })
            }
            DragMode::Absolute {
                range,
                fine_sensitivity,
            } => {
                if let Some(sensitivity) = fine_sensitivity.filter(|_| shift) {
                    Some(start + (delta.x - delta.y) * sensitivity)
                } else {
                    response
                        .interact_pointer_pos()
                        .map(|p| (p.x - range.left()) / range.width())
                        .or(fine_sensitivity.map(|_| normalized))
                }
            }
        };
        if let Some(value) = value {
            setter.set_parameter(param, from_normalized(value.clamp(0.0, 1.0)));
        }
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
}
