use super::*;

pub(super) fn control(
    ui: &mut egui::Ui,
    params: &WowParams,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    palette: Palette,
) {
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("PHASE")
                .size(oiko_ui::typography::TEXT_SMALL)
                .color(palette.muted),
        );
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(58.0), Sense::hover());
        let center = rect.center();
        let response = ui
            .interact(
                rect,
                Id::new(("knob", params.phase_offset.as_ptr())),
                Sense::click_and_drag(),
            )
            .on_hover_text(
                "Drag to rotate phase; Alt-drag for stereo spread. Shift for fine adjustment.",
            );
        dial_drag(ui, &response, params, setter);
        let phase_degrees = (params.phase_offset.value() * 360.0).rem_euclid(360.0);
        let spread_degrees = params.stereo.value() * 180.0;
        let phase = phase_degrees.to_radians() - PI * 0.5;
        let half_spread = spread_degrees.to_radians() * 0.5;
        ui.painter().circle_filled(center, 28.0, palette.page);
        ui.painter()
            .circle_stroke(center, 28.0, Stroke::new(1.0, palette.rule));
        let arc: Vec<Pos2> = (0..=48)
            .map(|i| {
                center
                    + Vec2::angled(phase - half_spread + 2.0 * half_spread * i as f32 / 48.0) * 31.0
            })
            .collect();
        if spread_degrees > 0.0 {
            ui.painter()
                .add(Shape::line(arc, Stroke::new(1.5, palette.orange)));
        }
        ui.painter().line_segment(
            [
                center + Vec2::angled(phase) * 7.0,
                center + Vec2::angled(phase) * 22.0,
            ],
            Stroke::new(2.0, palette.blue),
        );
        let (readout, _) = ui.allocate_exact_size(Vec2::new(90.0, 14.0), Sense::hover());
        ui.painter().text(
            Pos2::new(readout.left() + 30.0, readout.center().y),
            Align2::RIGHT_CENTER,
            format!("{phase_degrees:.0}°"),
            FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            palette.ink,
        );
        let phase_response = ui
            .interact(
                Rect::from_min_max(
                    readout.min,
                    Pos2::new(readout.left() + 34.0, readout.bottom()),
                ),
                Id::new(("phase-value", params.phase_offset.as_ptr())),
                Sense::click_and_drag(),
            )
            .on_hover_text(
                "Phase offset for both oscillators. Drag to rotate; Shift for fine adjustment.",
            );
        phase_drag(ui, &phase_response, &params.phase_offset, setter);
        let spread_response = ui
            .interact(
                Rect::from_min_max(Pos2::new(readout.left() + 37.0, readout.top()), readout.max),
                Id::new(("spread-value", params.stereo.as_ptr())),
                Sense::click_and_drag(),
            )
            .on_hover_text(
                "Stereo spread around the phase offset. Drag to adjust; Shift for fine adjustment.",
            );
        parameter_drag(
            ui,
            &spread_response,
            &params.stereo,
            setter,
            DragMode::Relative {
                sensitivity: 0.0045,
                horizontal_weight: 1.0,
                fine_scale: 0.2,
            },
        );
        if spread_response.double_clicked() {
            setter.set_discrete_parameter(&params.stereo, params.stereo.default_plain_value());
        }
        if spread_response.hovered() || spread_response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        for x in [43.5, 49.0] {
            ui.painter().circle_stroke(
                Pos2::new(readout.left() + x, readout.center().y + 1.0),
                3.5,
                Stroke::new(1.0, palette.orange),
            );
        }
        ui.painter().text(
            Pos2::new(readout.left() + 58.0, readout.center().y),
            Align2::LEFT_CENTER,
            format!("{spread_degrees:.0}°"),
            FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            palette.orange,
        );
    });
}

fn dial_drag(
    ui: &egui::Ui,
    response: &Response,
    params: &WowParams,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
) {
    let memory = response.id.with("spread-drag");
    let alt = ui.input(|input| input.modifiers.alt);
    // Choose the parameter on mouse-down and keep its host gesture intact even
    // if Alt changes before the drag threshold or before the mouse is released.
    if response.is_pointer_button_down_on() && ui.input(|input| input.pointer.primary_pressed()) {
        ui.data_mut(|data| data.insert_temp(memory, alt));
    }
    let spread = ui.data(|data| data.get_temp::<bool>(memory)).unwrap_or(alt);
    if spread {
        parameter_drag(
            ui,
            response,
            &params.stereo,
            setter,
            DragMode::Relative {
                sensitivity: 0.0045,
                horizontal_weight: 0.35,
                fine_scale: 0.2,
            },
        );
        if response.double_clicked() {
            setter.set_discrete_parameter(&params.stereo, params.stereo.default_plain_value());
        }
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
    } else {
        phase_drag(ui, response, &params.phase_offset, setter);
    }
    if !ui.input(|input| input.pointer.primary_down()) {
        ui.data_mut(|data| data.remove::<bool>(memory));
    }
}

// Keep the drag unwrapped until setting the circular parameter. Clamping the
// normalized drag, as for an ordinary knob, would prevent crossing zero.
fn phase_drag(
    ui: &egui::Ui,
    response: &Response,
    param: &nice_plug::params::FloatParam,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
) {
    let memory = response.id.with("phase-drag-start");
    if response.drag_started() {
        setter.begin_set_parameter(param);
        ui.data_mut(|data| data.insert_temp(memory, param.unmodulated_normalized_value()));
    }
    if response.dragged() {
        let start = ui
            .data(|data| data.get_temp::<f32>(memory))
            .unwrap_or(param.value());
        let delta = response.total_drag_delta().unwrap_or_default();
        let fine = if ui.input(|input| input.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let value = (start + (-delta.y + 0.35 * delta.x) * 0.0045 * fine).rem_euclid(1.0);
        setter.set_parameter(param, value);
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        setter.set_discrete_parameter(param, param.default_plain_value());
    }
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
}
