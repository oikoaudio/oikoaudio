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
    let memory_id = response.id.with("drag-start");
    let normalized = param.unmodulated_normalized_value();
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
            setter.set_parameter(param, param.preview_plain(value.clamp(0.0, 1.0)));
        }
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
}
