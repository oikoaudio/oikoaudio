//! Product-local icons and scale diagrams.
#[cfg(test)]
use super::trace;
use crate::editor_model::ScaleShape;
use crate::editor_theme::{Palette, TEXT_SMALL};
use egui::{Align2, FontId, Sense, Stroke, Vec2, pos2, vec2};

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Power(bool),
    Fork,
    Plus,
    Refresh,
    Trash,
    Gear,
    Close,
    SlotStatus(bool),
    Star(bool),
    Chevron(bool),
    Speaker(bool),
}
pub(super) fn icon(
    ui: &mut egui::Ui,
    id: &str,
    kind: Icon,
    tip: &str,
    p: Palette,
) -> egui::Response {
    sized_icon(ui, id, kind, tip, p, 23.)
}
pub(super) fn small_icon(
    ui: &mut egui::Ui,
    id: &str,
    kind: Icon,
    tip: &str,
    p: Palette,
) -> egui::Response {
    sized_icon(ui, id, kind, tip, p, 16.)
}
pub(super) fn sized_icon(
    ui: &mut egui::Ui,
    id: &str,
    kind: Icon,
    tip: &str,
    p: Palette,
    size: f32,
) -> egui::Response {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let response = ui
        .interact(r, ui.id().with(id), Sense::click())
        .on_hover_text(tip);
    if response.hovered() {
        ui.painter().rect_filled(r, 3, p.track);
    }
    let c = r.center();
    let color = if response.hovered() { p.ink } else { p.muted };
    let stroke = Stroke::new(1.3, color);
    let paint = ui.painter();
    let icon_scale = (if size < 23. { 0.8 } else { 1. })
        * if matches!(kind, Icon::Refresh) {
            0.75
        } else {
            1.
        };
    let icon_vec = |x: f32, y: f32| vec2(x * icon_scale, y * icon_scale);
    match kind {
        Icon::Power(on) => {
            let stroke = Stroke::new(1.6, if on { p.orange } else { p.muted });
            let points: Vec<_> = (0..25)
                .map(|i| {
                    c + Vec2::angled(
                        -std::f32::consts::FRAC_PI_2
                            + 0.65
                            + i as f32 / 24. * (std::f32::consts::TAU - 1.3),
                    ) * 6.
                })
                .collect();
            paint.add(egui::Shape::line(points, stroke));
            paint.line_segment([c + icon_vec(0., -8.), c + icon_vec(0., -1.)], stroke);
        }
        Icon::Fork => {
            paint.add(egui::Shape::line(
                vec![
                    c + icon_vec(-4., -7.),
                    c + icon_vec(-4., 0.),
                    c + icon_vec(-2., 3.),
                    c + icon_vec(2., 3.),
                    c + icon_vec(4., 0.),
                    c + icon_vec(4., -7.),
                ],
                stroke,
            ));
            paint.line_segment([c + icon_vec(0., 3.), c + icon_vec(0., 8.)], stroke);
        }

        Icon::Trash => {
            paint.line_segment([c + vec2(-5., -4.), c + vec2(5., -4.)], stroke);
            paint.line_segment([c + vec2(-2., -6.), c + vec2(2., -6.)], stroke);
            paint.add(egui::Shape::line(
                vec![
                    c + vec2(-4., -2.),
                    c + vec2(-3., 5.),
                    c + vec2(3., 5.),
                    c + vec2(4., -2.),
                ],
                stroke,
            ));
        }
        Icon::Plus => {
            let radius = if size < 23. { 6. } else { 5. };
            paint.line_segment([c - icon_vec(radius, 0.), c + icon_vec(radius, 0.)], stroke);
            paint.line_segment([c - icon_vec(0., radius), c + icon_vec(0., radius)], stroke);
        }
        Icon::Close => {
            paint.line_segment([c - icon_vec(4., 4.), c + icon_vec(4., 4.)], stroke);
            paint.line_segment([c + icon_vec(-4., 4.), c + icon_vec(4., -4.)], stroke);
        }
        Icon::SlotStatus(active) => {
            paint.circle_filled(c, 4., if active { p.orange } else { p.muted });
        }
        Icon::Gear => {
            let scale = icon_scale * if size < 23. { 0.9 } else { 1. };
            paint.circle_stroke(c, 4. * scale, stroke);
            for i in 0..8 {
                let d = Vec2::angled(i as f32 * std::f32::consts::TAU / 8.);
                paint.line_segment([c + d * (5. * scale), c + d * (7. * scale)], stroke);
            }
        }
        Icon::Refresh => {
            for offset in [0., std::f32::consts::PI] {
                let points: Vec<_> = (0..16)
                    .map(|i| c + Vec2::angled(offset + i as f32 / 15. * 2.3) * (6. * icon_scale))
                    .collect();
                let end = *points.last().unwrap();
                paint.add(egui::Shape::line(points, stroke));
                paint.line_segment(
                    [
                        end,
                        end + Vec2::angled(offset + 2.3 - 0.9) * (4. * icon_scale),
                    ],
                    stroke,
                );
                paint.line_segment(
                    [
                        end,
                        end + Vec2::angled(offset + 2.3 - 2.2) * (4. * icon_scale),
                    ],
                    stroke,
                );
            }
        }
        Icon::Star(on) => {
            let points: Vec<_> = (0..11)
                .map(|i| {
                    c + Vec2::angled(
                        i as f32 * std::f32::consts::TAU / 10. - std::f32::consts::FRAC_PI_2,
                    ) * if i % 2 == 0 { 6. } else { 2.7 }
                })
                .collect();
            paint.add(egui::Shape::line(
                points,
                Stroke::new(1.2, if on { p.orange } else { color }),
            ));
        }
        Icon::Speaker(on) => {
            let stroke = Stroke::new(1.3, if on { p.orange } else { p.muted });
            paint.add(egui::Shape::closed_line(
                vec![
                    c + icon_vec(-4., -2.),
                    c + icon_vec(0., -2.),
                    c + icon_vec(3., -5.),
                    c + icon_vec(3., 5.),
                    c + icon_vec(0., 2.),
                    c + icon_vec(-4., 2.),
                ],
                stroke,
            ));
        }

        Icon::Chevron(forward) => {
            let d = if forward { 1. } else { -1. };
            paint.add(egui::Shape::line(
                vec![
                    c + icon_vec(-2.5 * d, -4.5),
                    c + icon_vec(2.5 * d, 0.),
                    c + icon_vec(-2.5 * d, 4.5),
                ],
                Stroke::new(1.5, color),
            ));
        }
    }
    response
}
pub(super) fn draw_scale_sized(ui: &mut egui::Ui, shape: &ScaleShape, p: Palette, size: f32) {
    draw_scale_compared(ui, shape, p, size, false);
}
pub(super) fn draw_scale_compared(
    ui: &mut egui::Ui,
    shape: &ScaleShape,
    p: Palette,
    size: f32,
    reference: bool,
) {
    let caption_height = if size >= 100. { 30. } else { 0. };
    let (r, response) = ui.allocate_exact_size(vec2(size, size + caption_height), Sense::hover());
    #[cfg(test)]
    trace(ui, "scale-wheel", r);
    let c = r.min + vec2(size / 2., size / 2.);
    let radius = size * 54. / 138.;
    ui.painter()
        .circle_stroke(c, radius, Stroke::new(1., p.rule));
    let reference =
        size >= 100. && (reference || response.hovered()) && (shape.period_ratio - 2.).abs() < 1e-9;
    let mut caption = String::new();
    if reference {
        ui.painter()
            .circle_stroke(c, radius + 10., Stroke::new(0.7, p.rule));
        for n in 0..12 {
            let d =
                Vec2::angled(n as f32 * std::f32::consts::TAU / 12. - std::f32::consts::FRAC_PI_2);
            ui.painter().line_segment(
                [c + d * (radius + 7.), c + d * (radius + 13.)],
                Stroke::new(1.2, p.muted),
            );
        }
    }
    // Response positions are transformed from global host coordinates back to
    // this layer's local coordinates. Reading InputState directly here broke
    // spoke hover whenever the macOS content layer was scaled above 100%.
    let hovered_note = response
        .hover_pos()
        .filter(|pos| pos.distance(c) >= 20.)
        .and_then(|pos| {
            shape
                .positions
                .iter()
                .enumerate()
                .map(|(i, fraction)| {
                    let d = Vec2::angled(
                        *fraction as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2,
                    );
                    let along = (pos - c).dot(d).clamp(20., radius);
                    (i, pos.distance(c + d * along))
                })
                .filter(|(_, distance)| *distance <= 9.)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        });
    for (i, fraction) in shape.positions.iter().enumerate() {
        let d =
            Vec2::angled((*fraction as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2);
        let end = c + d * radius;
        if size >= 100. && hovered_note == Some(i) {
            let cents = fraction * 1200. * shape.period_ratio.log2();
            if reference {
                let nearest = (cents / 100.).round() * 100.;
                let difference = cents - nearest;
                let rd = Vec2::angled(
                    nearest as f32 / 1200. * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2,
                );
                ui.painter()
                    .line_segment([end, c + rd * (radius + 10.)], Stroke::new(1.5, p.orange));
                ui.painter()
                    .circle_stroke(end, 5., Stroke::new(1., p.orange));
                let interval = [
                    "Unison",
                    "Minor second",
                    "Major second",
                    "Minor third",
                    "Major third",
                    "Perfect fourth",
                    "Tritone",
                    "Perfect fifth",
                    "Minor sixth",
                    "Major sixth",
                    "Minor seventh",
                    "Major seventh",
                    "Octave",
                ][(nearest / 100.).round().clamp(0., 12.) as usize];
                caption = format!(
                    "Note {} · {interval}\n{}",
                    i + 1,
                    if difference.abs() < 0.05 {
                        "Matches 12 EDO".into()
                    } else {
                        format!(
                            "{:.1} cents {}",
                            difference.abs(),
                            if difference < 0. { "below" } else { "above" }
                        )
                    }
                );
            } else {
                ui.painter()
                    .circle_stroke(end, 5., Stroke::new(1., p.orange));
                caption = format!(
                    "Note {} · {:.1} cents\n≈{:.4}:1 from root",
                    i + 1,
                    cents,
                    shape.period_ratio.powf(*fraction)
                );
            }
            if i == 0 {
                caption = if reference {
                    "Root · 0 cents\nOne turn = one octave".into()
                } else {
                    format!("Root · 0 cents\nOne turn = {:.4}:1", shape.period_ratio)
                };
            }
        }

        ui.painter().line_segment(
            [c + d * 12., end],
            Stroke::new(
                if i == 0 { 1.8 } else { 1. },
                if i == 0 { p.orange } else { p.blue },
            ),
        );
        if shape.positions.len() < 80 {
            ui.painter().circle_filled(
                end,
                if i == 0 { 3. } else { 2. },
                if i == 0 { p.orange } else { p.blue },
            );
        }
    }
    if size >= 100. && (reference || !caption.is_empty()) {
        if caption.is_empty() {
            caption = "12 EDO reference\nHover a note to compare".into();
        }
        ui.painter().text(
            pos2(c.x, r.top() + size + 2.),
            Align2::CENTER_TOP,
            caption,
            FontId::proportional(TEXT_SMALL),
            p.muted,
        );
    }
    ui.painter().circle_filled(c, 3., p.muted);
}
