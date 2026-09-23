//! Interface scale is distinct from monitor DPI and from egui's zoom factor.
//! All dimensions here are logical points.
pub const SCALE_STEPS: [f64; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
pub const UI_SCALE_STEPS: [f32; 7] = [
    SCALE_STEPS[0] as f32,
    SCALE_STEPS[1] as f32,
    SCALE_STEPS[2] as f32,
    SCALE_STEPS[3] as f32,
    SCALE_STEPS[4] as f32,
    SCALE_STEPS[5] as f32,
    SCALE_STEPS[6] as f32,
];

/// Snap a persisted interface scale to the nearest supported step. Non-finite
/// values return the safe 100% default.
pub fn nearest_scale(scale: f64) -> f64 {
    if !scale.is_finite() {
        return 1.0;
    }
    SCALE_STEPS
        .into_iter()
        .min_by(|a, b| (a - scale).abs().total_cmp(&(b - scale).abs()))
        .unwrap_or(1.0)
}

pub fn closest_ui_scale(scale: f32) -> f32 {
    nearest_scale(scale as f64) as f32
}

/// Canvas transform needed in addition to egui's zoom factor on the current
/// platform.
pub fn canvas_scale(scale: f32) -> f32 {
    if cfg!(target_os = "macos") {
        closest_ui_scale(scale)
    } else {
        1.0
    }
}

/// Apply the restored interface scale whenever an editor window opens. Hosts
/// can construct editors before restoring state, so the scale known at
/// construction may be stale.
pub fn initialize_scale(context: &egui::Context, scale: f32, logical_size: egui::Vec2) {
    request_scale(context, scale, logical_size);
}

/// Queue a resize through egui, never call the host from inside an egui callback.
/// The backend services viewport commands after the UI pass has completed.
pub fn request_scale(context: &egui::Context, scale: f32, logical_size: egui::Vec2) {
    let scale = closest_ui_scale(scale);
    context.set_zoom_factor(if cfg!(target_os = "macos") {
        1.0
    } else {
        scale
    });
    if cfg!(target_os = "macos") {
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(logical_size * scale));
    }
    context.request_repaint();
}

#[cfg(test)]
mod tests;
