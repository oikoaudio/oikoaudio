//! The shared NicePlug adapter. DSP crates must never depend on this crate.
pub mod scale_state;
pub use scale_state::UiScaleState;

/// Create persisted editor geometry with one platform convention for all products.
/// `logical_size` is the unscaled canvas in points, independent of monitor DPI.
pub fn editor_state(
    logical_size: egui::Vec2,
    scale: f32,
) -> std::sync::Arc<nice_plug_egui::EguiEditorState> {
    let scale = oiko_ui::scale::closest_ui_scale(scale);
    let (size, zoom) = if cfg!(target_os = "macos") {
        (logical_size * scale, 1.0)
    } else {
        (logical_size, scale)
    };
    nice_plug_egui::EguiEditorState::from_size(
        nice_plug::editor::dpi::LogicalSize {
            width: size.x,
            height: size.y,
        },
        zoom,
    )
}

pub mod gestures;

pub mod parameter_controls;
