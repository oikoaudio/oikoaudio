//! Parameter interaction only. Products own painting, hit rectangles and shortcuts.
use crate::gestures::ParameterWriter;
use egui::{Rect, Response, Ui};
use nice_plug::params::Param;

/// How pointer movement maps to a normalized parameter value.
#[derive(Clone, Copy)]
pub enum DragMode {
    /// Moves the value from where the drag started.
    Relative {
        /// Normalized change per point of upward movement.
        sensitivity: f32,
        /// Contribution of rightward movement relative to upward movement.
        horizontal_weight: f32,
        /// Sensitivity multiplier while Shift is held.
        fine_scale: f32,
    },
    /// Sets the value from the pointer's horizontal position.
    Absolute {
        /// Pointer span mapped to 0..1, left to right.
        range: Rect,
        /// With Shift held, moves the value from where the drag started by
        /// this normalized change per point (rightward or upward). `None`
        /// disables fine adjustment.
        fine_sensitivity: Option<f32>,
    },
}

/// Wraps a drag in one host gesture and sets `param` according to `mode`.
/// Gestures interrupted by focus loss or editor close are ended by
/// `ParameterGestures::finish`.
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

/// `parameter_drag` for a control whose visual scale differs from the
/// parameter's own normalization. `mapping` holds the current display-space
/// position (0..1) and a function from a display-space position to the
/// parameter's plain value.
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
