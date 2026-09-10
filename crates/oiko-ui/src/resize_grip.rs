//! A corner drag selects a supported zoom; the caller commits it on release.
use egui::{
    Align2, CursorIcon, FontId, Id, LayerId, Order, Pos2, Rect, Sense, Ui, UiBuilder, Vec2,
};

use crate::scale::{canvas_scale, closest_ui_scale};

/// Space reserved at the right of each product footer, in canvas points.
pub const RESERVED_WIDTH: f32 = 24.0;

#[derive(Clone, Copy)]
struct Drag {
    origin: Pos2,
    scale: f32,
    target: f32,
    canvas: Vec2,
}

impl Drag {
    fn update(&mut self, position: Pos2) {
        let scale =
            self.scale + (position - self.origin).dot(self.canvas) / self.canvas.length_sq();
        // A small dead band keeps the preview stable at a step boundary.
        if (scale - self.target).abs() > 0.145 {
            self.target = closest_ui_scale(scale);
        }
    }
}

/// Draw on the untransformed root UI, after the product's content. Returns a new
/// zoom only on release. No viewport commands are issued while dragging: changing
/// the window or egui zoom then would move the pointer's coordinate reference.
pub fn show(root: &mut Ui, canvas: Vec2, scale: f32) -> Option<f32> {
    let id = Id::new("oiko-resize-grip");
    let context = root.ctx().clone();
    let zoom = context.zoom_factor();
    let render_scale = canvas_scale(scale);
    let viewport = context.content_rect();
    let rect = Rect::from_min_max(
        viewport.max - Vec2::splat(22.0 * render_scale),
        viewport.max,
    );
    let mut ui = root.new_child(
        UiBuilder::new()
            .id_salt(id)
            .layer_id(LayerId::new(Order::Foreground, id))
            .max_rect(rect),
    );
    ui.set_clip_rect(viewport);
    let response = ui.interact(rect, id, Sense::drag());
    response
        .clone()
        .on_hover_cursor(CursorIcon::ResizeNwSe)
        .on_hover_text("Drag to resize; release to apply");
    let mut drag = context.data_mut(|data| data.get_temp::<Drag>(id));
    if response.drag_started()
        && let Some(origin) = context.input(|input| input.pointer.press_origin())
    {
        drag = Some(Drag {
            origin: origin * zoom,
            scale,
            target: scale,
            canvas,
        });
    }

    let mut selected = None;
    if let Some(active) = &mut drag {
        let (position, down, focused, escape) = context.input(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.primary_down(),
                input.focused,
                input.key_pressed(egui::Key::Escape),
            )
        });
        if !focused || escape || active.scale != scale || active.canvas != canvas {
            drag = None;
        } else {
            if let Some(position) = position {
                // Monitor-independent window points, unaffected by canvas transforms.
                active.update(position * zoom);
            }
            if response.drag_stopped() {
                if position.is_some() && active.target != scale {
                    selected = Some(active.target);
                }
                drag = None;
            } else if !down {
                drag = None;
            }
        }
    }

    let stroke = ui.style().interact(&response).fg_stroke;
    let corner = rect.max - Vec2::splat(4.0 * render_scale);
    for length in [4.0, 8.0, 12.0] {
        let length = length * render_scale;
        ui.painter().line_segment(
            [
                corner - Vec2::new(length, 0.0),
                corner - Vec2::new(0.0, length),
            ],
            stroke,
        );
    }
    if let Some(active) = drag {
        context.set_cursor_icon(CursorIcon::ResizeNwSe);
        paint_preview(&ui, viewport, active);
        context.data_mut(|data| data.insert_temp(id, active));
    } else {
        context.data_mut(|data| data.remove::<Drag>(id));
    }
    selected
}

fn paint_preview(ui: &Ui, viewport: Rect, drag: Drag) {
    let palette = crate::theme::Palette::new(ui.visuals().dark_mode);
    // Keep the readout legible at any user zoom, fitting even Inton's 50% window.
    let zoom = ui.ctx().zoom_factor();
    let available = viewport.size() * zoom - Vec2::splat(24.0);
    let unit = (available.x / 240.0).min(available.y / 104.0).min(1.0) / zoom;
    let panel = Rect::from_center_size(viewport.center(), Vec2::new(240.0, 104.0) * unit);
    let painter = ui.painter();
    painter.rect_filled(panel, 8.0, palette.panel);
    painter.rect_stroke(
        panel,
        8.0,
        egui::Stroke::new(unit, palette.rule),
        egui::StrokeKind::Inside,
    );
    let at = |x, y| panel.min + Vec2::new(x, y) * unit;
    painter.text(
        at(120.0, 40.0),
        Align2::CENTER_CENTER,
        format!("{:.0}%", drag.target * 100.0),
        FontId::proportional(crate::typography::TEXT_TITLE * 1.8 * unit),
        palette.ink,
    );

    painter.text(
        at(120.0, 82.0),
        Align2::CENTER_CENTER,
        "Release to apply · Esc to cancel",
        FontId::proportional(crate::typography::TEXT_SMALL * unit),
        palette.muted,
    );
}

#[cfg(test)]
mod tests;
