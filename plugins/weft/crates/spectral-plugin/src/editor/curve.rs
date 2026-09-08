//! Curve and note-ruler interaction, editing and transforms.
use super::controls::spectrum_range_selector;
use super::history::CurveHistory;
use super::rendering::{
    LineTrail, PlotContext, active_bin_master_span, active_bin_to_master_index, clamp_to_rect,
    curve_db_to_y, draw_axis_labels, draw_axis_veil, draw_grid_lines, draw_horizontal_shift_cursor,
    draw_hovered_bin, draw_input_notes, draw_motion_outline, draw_note_skirts, draw_plot_labels,
    draw_spectrum, draw_transform_edge_glow, draw_transform_handles,
    draw_vertical_transform_cursor, draw_visible_bin_boundaries, frequency_to_x, midi_note_octave,
    mix_color, ruler_intro_strength, with_alpha, x_to_active_bin, y_to_curve_db,
};
use crate::curve::transformed_curve_db;
use crate::display_data::{ANALYZER_POINTS, AnalysisDisplay, display_max_frequency};
use crate::parameters::SpectralParams;
use crate::state::{CurveState, PinnedNotesState};
use egui::emath::GuiRounding;
use egui::{Align2, Color32, FontId, Id, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use oiko_dsp::note_frequency;
use oiko_ui::theme::Palette;
use spectral_dsp::{
    MANUAL_CURVE_MUTE_DB, MANUAL_MASK_POINTS, MIDI_NOTES, MIN_DISPLAY_FREQUENCY_HZ,
};
use std::f32::consts::PI;

pub(super) const SOFT_PENCIL_RADIUS_PX: f32 = 12.0;
#[derive(Clone, Copy)]
pub(super) struct EditPoint {
    pub(super) active_bin: usize,
    pub(super) db: f32,
}

#[derive(Clone, Copy)]
pub(super) enum CurveAction {
    Reset,
    ToggleCapture,
    Flip,
    Copy,
    Paste,
    Save,
    Load,
    BeginEdit,
    CommitTransform(CurveTransformDrag),
    Undo,
    Redo,
}

pub(super) struct CurveEditorOutput {
    pub(super) action: Option<CurveAction>,
    pub(super) rect: Rect,
}

pub(super) fn curve_history_shortcut(
    context: &egui::Context,
    history: &CurveHistory,
    curve_shortcuts_active: bool,
) -> Option<CurveAction> {
    if !curve_shortcuts_active || context.text_edit_focused() {
        return None;
    }

    let redo =
        egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z);
    let redo_requested = context.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: Key::Z,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.matches_logically(egui::Modifiers::COMMAND)
                    && modifiers.shift
            )
        })
    });
    if redo_requested {
        if history.is_active() {
            let _ = context.input_mut(|input| input.consume_shortcut(&redo));
            return history.can_redo().then_some(CurveAction::Redo);
        }
        return None;
    }

    let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::Z);
    if history.is_active() && context.input_mut(|input| input.consume_shortcut(&undo)) {
        return history.can_undo().then_some(CurveAction::Undo);
    }
    None
}

#[derive(Clone, Copy)]
pub(super) enum CurveTransformDrag {
    Depth(f32),
    TiltLow(f32),
    TiltHigh(f32),
    Shift(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CurveTransformZone {
    Depth,
    TiltLow,
    TiltHigh,
    Shift,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn curve_editor(
    ui: &mut egui::Ui,
    curve: &CurveState,
    context: PlotContext<'_>,
    last_curve_point: &mut Option<EditPoint>,
    orange_motion_trail: &mut LineTrail,
    blue_motion_trail: &mut LineTrail,
    spectrum_range: &crate::state::SpectrumRangeState,
    spectrum_range_menu_open: &mut bool,
    capture_active: bool,
    capture_preview: Option<&CurveState>,
    capture_seconds: f32,
    transform_mode: &mut bool,
    transform_drag: &mut Option<CurveTransformDrag>,
    suppress_alt_transform: bool,
    surface_blocked: bool,
    can_undo: bool,
    can_redo: bool,
    params: &SpectralParams,
    setter: &nice_plug::context::gui::ParamSetter<'_>,
) -> CurveEditorOutput {
    let desired = Vec2::new(ui.available_width(), 292.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, context.palette.page);

    let plot = rect.shrink2(Vec2::new(10.0, 14.0));
    draw_spectrum(&painter, plot, &context);
    draw_grid_lines(
        &painter,
        plot,
        context.sample_rate,
        context.display_range_db,
        context.palette,
    );
    draw_visible_bin_boundaries(
        &painter,
        plot,
        context.sample_rate,
        context.fft_size,
        context.palette,
    );
    draw_input_notes(
        &painter,
        plot,
        context.display,
        context.sample_rate,
        context.palette,
    );
    draw_motion_outline(&painter, plot, &context, orange_motion_trail);
    draw_note_skirts(&painter, plot, &context, blue_motion_trail);
    draw_axis_veil(&painter, plot, context.palette);
    draw_axis_labels(
        &painter,
        plot,
        context.sample_rate,
        context.display_range_db,
        context.palette,
    );
    draw_plot_labels(&painter, plot, context.note_control_mix, context.palette);
    if let Some(preview) = capture_preview {
        let mut values = [0.0; MANUAL_MASK_POINTS];
        preview.copy_to(&mut values);
        let points: Vec<Pos2> = (0..=plot.width().ceil() as usize)
            .map(|column| {
                let x = plot.left() + column as f32;
                let bin = x_to_active_bin(x, plot, context.sample_rate, context.fft_size);
                let index = active_bin_to_master_index(bin, context.fft_size);
                Pos2::new(
                    x,
                    curve_db_to_y(values[index], plot, context.display_range_db),
                )
            })
            .collect();
        painter.with_clip_rect(plot).add(egui::Shape::line(
            points,
            Stroke::new(1.2, with_alpha(context.palette.orange, 110)),
        ));
    }
    if capture_active {
        painter.text(
            Pos2::new(plot.center().x, plot.top() + 8.0),
            Align2::CENTER_TOP,
            format!("CAPTURING {capture_seconds:.1}s"),
            FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            context.palette.muted,
        );
    }

    let action_button_height = 20.0;
    let action_button_font = FontId::proportional(oiko_ui::typography::TEXT_SMALL);
    let reset_width = compact_action_button_width(
        &painter,
        &["RESET"],
        &action_button_font,
        action_button_height,
    );
    let reset_rect = Rect::from_min_size(
        Pos2::new(plot.left() + 4.0, plot.bottom() - 38.0),
        Vec2::new(reset_width, action_button_height),
    );
    let reset = ui.interact(reset_rect, Id::new("reset-curve"), Sense::click());
    let mut action = reset.clicked().then_some(CurveAction::Reset);
    if reset.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.rect_filled(
        reset_rect,
        3.0,
        if reset.hovered() {
            context.palette.track
        } else {
            context.palette.page
        },
    );
    painter.rect_stroke(
        reset_rect,
        3.0,
        Stroke::new(1.0, context.palette.rule),
        StrokeKind::Inside,
    );
    painter.text(
        reset_rect.center(),
        Align2::CENTER_CENTER,
        "RESET",
        action_button_font.clone(),
        context.palette.muted,
    );

    let capture_width = compact_action_button_width(
        &painter,
        &["CAPTURE"],
        &action_button_font,
        action_button_height,
    );
    let capture_rect = Rect::from_min_size(
        Pos2::new(reset_rect.right() + 6.0, reset_rect.top()),
        Vec2::new(capture_width, action_button_height),
    );
    let capture = ui.interact(capture_rect, Id::new("capture-curve"), Sense::click());
    if capture.clicked() {
        action = Some(CurveAction::ToggleCapture);
    }
    if capture.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.rect_filled(
        capture_rect,
        3.0,
        if capture_active {
            with_alpha(context.palette.orange, 28)
        } else if capture.hovered() {
            context.palette.track
        } else {
            context.palette.page
        },
    );

    painter.rect_stroke(
        capture_rect,
        3.0,
        Stroke::new(
            1.0,
            if capture_active {
                context.palette.orange
            } else {
                context.palette.rule
            },
        ),
        StrokeKind::Inside,
    );
    painter.text(
        capture_rect.center(),
        Align2::CENTER_CENTER,
        "CAPTURE",
        action_button_font.clone(),
        if capture_active {
            context.palette.orange
        } else {
            context.palette.muted
        },
    );

    let flip_width = compact_action_button_width(
        &painter,
        &["FLIP"],
        &action_button_font,
        action_button_height,
    );
    let flip_rect = Rect::from_min_size(
        Pos2::new(capture_rect.right() + 6.0, capture_rect.top()),
        Vec2::new(flip_width, action_button_height),
    );
    let flip = ui.interact(flip_rect, Id::new("flip-curve"), Sense::click());
    if flip.clicked() && !capture_active {
        action = Some(CurveAction::Flip);
    }
    if flip.hovered() && !capture_active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.rect_filled(
        flip_rect,
        3.0,
        if flip.hovered() && !capture_active {
            context.palette.track
        } else {
            context.palette.page
        },
    );
    painter.rect_stroke(
        flip_rect,
        3.0,
        Stroke::new(1.0, context.palette.rule),
        StrokeKind::Inside,
    );
    painter.text(
        flip_rect.center(),
        Align2::CENTER_CENTER,
        "FLIP",
        action_button_font.clone(),
        if capture_active {
            context.palette.rule
        } else {
            context.palette.muted
        },
    );

    let transform_width = compact_action_button_width(
        &painter,
        &["TRANSFORM"],
        &action_button_font,
        action_button_height,
    );
    let transform_rect = Rect::from_min_size(
        Pos2::new(flip_rect.right() + 6.0, flip_rect.top()),
        Vec2::new(transform_width, action_button_height),
    );
    let transform_button = ui.interact(transform_rect, Id::new("transform-curve"), Sense::click());
    if transform_button.clicked() && !capture_active {
        *transform_mode = !*transform_mode;
    }
    if transform_button.hovered() && !capture_active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.rect_filled(
        transform_rect,
        3.0,
        if capture_active {
            context.palette.page
        } else if *transform_mode {
            with_alpha(context.palette.orange, 24)
        } else if transform_button.hovered() {
            context.palette.track
        } else {
            context.palette.page
        },
    );
    painter.rect_stroke(
        transform_rect,
        3.0,
        Stroke::new(
            1.0,
            if capture_active {
                context.palette.rule
            } else if *transform_mode {
                context.palette.orange
            } else {
                context.palette.rule
            },
        ),
        StrokeKind::Inside,
    );
    painter.text(
        transform_rect.center(),
        Align2::CENTER_CENTER,
        "TRANSFORM",
        action_button_font,
        if capture_active {
            context.palette.rule
        } else if *transform_mode {
            context.palette.orange
        } else {
            context.palette.muted
        },
    );

    let transfer_rect = Rect::from_min_size(
        Pos2::new(transform_rect.right() + 6.0, transform_rect.top()),
        Vec2::new(54.0, 20.0),
    );
    let transfer = ui.interact(transfer_rect, Id::new("curve-transfer"), Sense::click());
    painter.rect_filled(
        transfer_rect,
        3.0,
        if transfer.hovered() {
            context.palette.track
        } else {
            context.palette.page
        },
    );
    painter.rect_stroke(
        transfer_rect,
        3.0,
        Stroke::new(1.0, context.palette.rule),
        StrokeKind::Inside,
    );
    painter.text(
        transfer_rect.center() - Vec2::new(4.0, 0.0),
        Align2::CENTER_CENTER,
        "CURVE",
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        context.palette.muted,
    );
    let arrow = Pos2::new(transfer_rect.right() - 7.0, transfer_rect.center().y);
    painter.line(
        vec![
            arrow - Vec2::new(2.0, 1.0),
            arrow + Vec2::new(0.0, 1.0),
            arrow + Vec2::new(2.0, -1.0),
        ],
        Stroke::new(1.0, context.palette.muted),
    );
    if transfer.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let transfer_menu = egui::Popup::menu(&transfer).show(|ui| {
        ui.style_mut().override_font_id =
            Some(FontId::proportional(oiko_ui::typography::TEXT_SMALL));
        if ui.button("Copy curve").clicked() {
            action = Some(CurveAction::Copy);
            ui.close();
        }
        if ui
            .add_enabled(!capture_active, egui::Button::new("Paste curve"))
            .clicked()
        {
            action = Some(CurveAction::Paste);
            ui.close();
        }
        ui.separator();
        if ui.button("Save curve…").clicked() {
            action = Some(CurveAction::Save);
            ui.close();
        }
        if ui
            .add_enabled(!capture_active, egui::Button::new("Load curve…"))
            .clicked()
        {
            action = Some(CurveAction::Load);
            ui.close();
        }
    });
    let transfer_hovered = transfer.hovered() || transfer_menu.is_some();

    let undo_rect = Rect::from_min_size(
        Pos2::new(transfer_rect.right() + 6.0, transfer_rect.top()),
        Vec2::new(28.0, 20.0),
    );
    let undo = ui.interact(undo_rect, Id::new("undo-curve"), Sense::click());
    if undo.clicked() && can_undo {
        action = Some(CurveAction::Undo);
    }
    curve_history_button(
        ui,
        &painter,
        undo_rect,
        false,
        can_undo,
        undo.hovered(),
        context.palette,
    );

    let redo_rect = Rect::from_min_size(
        Pos2::new(undo_rect.right() + 6.0, undo_rect.top()),
        Vec2::new(28.0, 20.0),
    );
    let redo = ui.interact(redo_rect, Id::new("redo-curve"), Sense::click());
    if redo.clicked() && can_redo {
        action = Some(CurveAction::Redo);
    }
    curve_history_button(
        ui,
        &painter,
        redo_rect,
        true,
        can_redo,
        redo.hovered(),
        context.palette,
    );

    let alt_held = ui.input(|input| input.modifiers.alt);
    let transform_active = transform_interaction_active(
        capture_active,
        *transform_mode,
        alt_held,
        transform_drag.is_some(),
        suppress_alt_transform,
    ) && !surface_blocked;
    let interrupted_drag = (!transform_active).then(|| transform_drag.take()).flatten();
    let (transform_hovered, transform_stopped) = if transform_active {
        curve_transform_frame(
            ui,
            &painter,
            plot,
            &response,
            reset.hovered()
                || capture.hovered()
                || flip.hovered()
                || transfer_hovered
                || transform_button.hovered()
                || undo.hovered()
                || redo.hovered(),
            &context,
            transform_drag,
            params,
            setter,
        )
    } else {
        (false, false)
    };
    if transform_stopped && let Some(active_drag) = transform_drag.take() {
        action = Some(CurveAction::CommitTransform(active_drag));
    } else if let Some(active_drag) = interrupted_drag {
        action = Some(CurveAction::CommitTransform(active_drag));
    }

    let range_hovered = spectrum_range_selector(
        ui,
        Pos2::new(plot.left() + 2.0, plot.top() - 13.0),
        context.palette,
        spectrum_range,
        spectrum_range_menu_open,
    );

    if let Some(pointer) = response
        .hover_pos()
        .filter(|pointer| plot.contains(*pointer))
        .filter(|_| {
            !surface_blocked
                && !transform_active
                && !reset.hovered()
                && !capture.hovered()
                && !flip.hovered()
                && !transfer_hovered
                && !transform_button.hovered()
                && !undo.hovered()
                && !redo.hovered()
                && !range_hovered
        })
    {
        let soft_pencil = ui.input(|input| input.modifiers.shift);
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        let active_bin = x_to_active_bin(pointer.x, plot, context.sample_rate, context.fft_size);
        draw_hovered_bin(&painter, plot, pointer, active_bin, soft_pencil, &context);
    }

    if !reset.hovered()
        && !capture.hovered()
        && !flip.hovered()
        && !transfer_hovered
        && !transform_button.hovered()
        && !undo.hovered()
        && !redo.hovered()
        && !transform_hovered
        && !range_hovered
        && !transform_active
        && !surface_blocked
        && (response.dragged() || response.clicked())
        && let Some(raw_pointer) = response.interact_pointer_pos()
        && (response.dragged() || plot.expand(8.0).contains(raw_pointer))
    {
        if response.drag_started() || response.clicked() {
            action = Some(CurveAction::BeginEdit);
        }
        let pointer = clamp_to_rect(raw_pointer, plot);
        let (active_bin, db) = inverse_transformed_edit_point(pointer, plot, &context);
        let current = EditPoint { active_bin, db };
        if ui.input(|input| input.modifiers.shift) {
            let radius = soft_pencil_radius_bins(pointer.x, plot, &context);
            edit_with_soft_bins(curve, *last_curve_point, current, context.fft_size, radius);
        } else {
            edit_with_bins(curve, *last_curve_point, current, context.fft_size);
        }
        *last_curve_point = Some(current);
    } else if !response.dragged() {
        *last_curve_point = None;
    }
    CurveEditorOutput { action, rect }
}

pub(super) fn transform_interaction_active(
    capture_active: bool,
    transform_mode: bool,
    alt_held: bool,
    transform_drag_active: bool,
    suppress_alt_transform: bool,
) -> bool {
    !capture_active
        && (transform_mode
            || (!suppress_alt_transform && alt_held)
            || (!suppress_alt_transform && transform_drag_active))
}

pub(super) fn compact_action_button_width(
    painter: &egui::Painter,
    labels: &[&str],
    font: &FontId,
    height: f32,
) -> f32 {
    labels.iter().fold(0.0_f32, |width, label| {
        let galley = painter.layout_no_wrap((*label).to_owned(), font.clone(), Color32::WHITE);
        let padding = ((height - galley.size().y) * 0.5).max(4.0);
        width.max(galley.size().x + padding * 2.0)
    })
}

pub(super) fn curve_history_button(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    points_right: bool,
    enabled: bool,
    hovered: bool,
    palette: Palette,
) {
    if hovered && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.rect_filled(
        rect,
        3.0,
        if hovered && enabled {
            palette.track
        } else {
            palette.page
        },
    );
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, palette.rule),
        StrokeKind::Inside,
    );
    let color = if enabled { palette.muted } else { palette.rule };
    let direction = if points_right { 1.0 } else { -1.0 };
    let center = rect.center();
    let tip = Pos2::new(center.x + direction * 6.0, center.y - 1.0);
    let shoulder_x = tip.x - direction * 4.0;
    painter.line(
        vec![
            tip,
            Pos2::new(center.x - direction * 4.0, center.y - 1.0),
            Pos2::new(center.x - direction * 7.0, center.y + 2.0),
            Pos2::new(center.x - direction * 7.0, center.y + 6.0),
        ],
        Stroke::new(1.2, color),
    );
    painter.line_segment(
        [tip, Pos2::new(shoulder_x, tip.y - 3.5)],
        Stroke::new(1.2, color),
    );
    painter.line_segment(
        [tip, Pos2::new(shoulder_x, tip.y + 3.5)],
        Stroke::new(1.2, color),
    );
}

pub(super) fn set_curve_transform_defaults(
    params: &SpectralParams,
    setter: &nice_plug::context::gui::ParamSetter<'_>,
) {
    for (param, value) in [
        (&params.curve_depth_percent, 100.0),
        (&params.curve_tilt_db_per_octave, 0.0),
        (&params.curve_shift_semitones, 0.0),
    ] {
        if (param.value() - value).abs() > f32::EPSILON {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, value);
            setter.end_set_parameter(param);
        }
    }
}

pub(super) fn curve_transform_is_neutral(params: &SpectralParams) -> bool {
    (params.curve_depth_percent.value() - 100.0).abs() <= f32::EPSILON
        && params.curve_tilt_db_per_octave.value().abs() <= f32::EPSILON
        && params.curve_shift_semitones.value().abs() <= f32::EPSILON
}

pub(super) fn finish_curve_transform_drag(
    active: CurveTransformDrag,
    params: &SpectralParams,
    setter: &nice_plug::context::gui::ParamSetter<'_>,
) {
    let (param, value) = match active {
        CurveTransformDrag::Depth(_) => (&params.curve_depth_percent, 100.0),
        CurveTransformDrag::TiltLow(_) | CurveTransformDrag::TiltHigh(_) => {
            (&params.curve_tilt_db_per_octave, 0.0)
        }
        CurveTransformDrag::Shift(_) => (&params.curve_shift_semitones, 0.0),
    };
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

pub(super) fn cancel_curve_transform_drag(
    active: CurveTransformDrag,
    params: &SpectralParams,
    setter: &nice_plug::context::gui::ParamSetter<'_>,
) {
    let (param, initial) = match active {
        CurveTransformDrag::Depth(initial) => (&params.curve_depth_percent, initial),
        CurveTransformDrag::TiltLow(initial) | CurveTransformDrag::TiltHigh(initial) => {
            (&params.curve_tilt_db_per_octave, initial)
        }
        CurveTransformDrag::Shift(initial) => (&params.curve_shift_semitones, initial),
    };
    setter.set_parameter(param, initial);
    setter.end_set_parameter(param);
}

pub(super) fn apply_curve_transform(
    source: &[f32; MANUAL_MASK_POINTS],
    curve: &CurveState,
    sample_rate: f32,
    depth_percent: f32,
    tilt_db_per_octave: f32,
    shift_semitones: f32,
) {
    for output_index in 0..MANUAL_MASK_POINTS {
        curve.set_transformed(
            output_index,
            transformed_curve_db(
                source,
                output_index,
                sample_rate,
                depth_percent,
                tilt_db_per_octave,
                shift_semitones,
            ),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn curve_transform_frame(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    plot: Rect,
    surface_response: &egui::Response,
    block_surface: bool,
    context: &PlotContext<'_>,
    drag: &mut Option<CurveTransformDrag>,
    params: &SpectralParams,
    setter: &nice_plug::context::gui::ParamSetter<'_>,
) -> (bool, bool) {
    let frame = plot.shrink(7.0);
    let hovered_zone = (!block_surface)
        .then(|| {
            surface_response
                .hover_pos()
                .filter(|pointer| plot.contains(*pointer))
                .map(|pointer| curve_transform_zone(pointer, frame))
        })
        .flatten();
    let active_zone = drag.as_ref().map(|drag| match drag {
        CurveTransformDrag::Depth(_) => CurveTransformZone::Depth,
        CurveTransformDrag::TiltLow(_) => CurveTransformZone::TiltLow,
        CurveTransformDrag::TiltHigh(_) => CurveTransformZone::TiltHigh,
        CurveTransformDrag::Shift(_) => CurveTransformZone::Shift,
    });
    let visible_zone = active_zone.or(hovered_zone);

    painter.rect_stroke(
        frame,
        1.0,
        Stroke::new(1.0, with_alpha(context.palette.orange, 92)),
        StrokeKind::Inside,
    );
    if let Some(
        zone @ (CurveTransformZone::Depth
        | CurveTransformZone::TiltLow
        | CurveTransformZone::TiltHigh),
    ) = visible_zone
    {
        draw_transform_edge_glow(painter, frame, zone, context.palette);
    }
    draw_transform_handles(painter, frame, visible_zone, context.palette);

    let drag_start_zone = (!block_surface && surface_response.drag_started())
        .then(|| {
            surface_response
                .interact_pointer_pos()
                .filter(|pointer| plot.contains(*pointer))
                .map(|pointer| curve_transform_zone(pointer, frame))
        })
        .flatten();
    if let Some(zone) = drag_start_zone {
        *drag = Some(match zone {
            CurveTransformZone::Depth => {
                setter.begin_set_parameter(&params.curve_depth_percent);
                CurveTransformDrag::Depth(params.curve_depth_percent.value())
            }
            CurveTransformZone::TiltLow => {
                setter.begin_set_parameter(&params.curve_tilt_db_per_octave);
                CurveTransformDrag::TiltLow(params.curve_tilt_db_per_octave.value())
            }
            CurveTransformZone::TiltHigh => {
                setter.begin_set_parameter(&params.curve_tilt_db_per_octave);
                CurveTransformDrag::TiltHigh(params.curve_tilt_db_per_octave.value())
            }
            CurveTransformZone::Shift => {
                setter.begin_set_parameter(&params.curve_shift_semitones);
                CurveTransformDrag::Shift(params.curve_shift_semitones.value())
            }
        });
    }
    if surface_response.dragged() {
        let delta = surface_response.total_drag_delta().unwrap_or_default();
        match *drag {
            Some(CurveTransformDrag::Depth(initial)) => setter.set_parameter(
                &params.curve_depth_percent,
                curve_depth_from_drag(initial, delta.y, frame.height()),
            ),
            Some(CurveTransformDrag::TiltLow(initial)) => setter.set_parameter(
                &params.curve_tilt_db_per_octave,
                initial + delta.y / frame.height() * 24.0,
            ),
            Some(CurveTransformDrag::TiltHigh(initial)) => setter.set_parameter(
                &params.curve_tilt_db_per_octave,
                initial - delta.y / frame.height() * 24.0,
            ),
            Some(CurveTransformDrag::Shift(initial)) => setter.set_parameter(
                &params.curve_shift_semitones,
                initial + delta.x / frame.width() * 120.0,
            ),
            None => {}
        }
    }

    match visible_zone {
        Some(
            CurveTransformZone::Depth | CurveTransformZone::TiltLow | CurveTransformZone::TiltHigh,
        ) => {
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            if let Some(pointer) = transform_pointer(surface_response, plot) {
                draw_vertical_transform_cursor(painter, pointer, context.palette);
            }
        }
        Some(CurveTransformZone::Shift) => {
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            if let Some(pointer) = transform_pointer(surface_response, plot) {
                draw_horizontal_shift_cursor(painter, pointer, context.palette);
            }
        }
        None => {}
    }

    if let Some(active) = *drag {
        let text = match active {
            CurveTransformDrag::Depth(_) => {
                format!("CURVE DEPTH {:.0}%", params.curve_depth_percent.value())
            }
            CurveTransformDrag::TiltLow(_) | CurveTransformDrag::TiltHigh(_) => format!(
                "TILT {:+.1} dB/oct",
                params.curve_tilt_db_per_octave.value()
            ),
            CurveTransformDrag::Shift(_) => {
                format!("SHIFT {:+.1} st", params.curve_shift_semitones.value())
            }
        };
        painter.text(
            frame.center_top() + Vec2::new(0.0, 9.0),
            Align2::CENTER_TOP,
            text,
            FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            context.palette.orange,
        );
    }
    let stopped = drag.is_some() && surface_response.drag_stopped();
    (
        visible_zone.is_some() || surface_response.dragged(),
        stopped,
    )
}

pub(super) fn curve_transform_zone(pointer: Pos2, frame: Rect) -> CurveTransformZone {
    const EDGE_ZONE: f32 = 20.0;
    if pointer.y >= frame.bottom() - EDGE_ZONE {
        CurveTransformZone::Depth
    } else if pointer.x <= frame.left() + EDGE_ZONE {
        CurveTransformZone::TiltLow
    } else if pointer.x >= frame.right() - EDGE_ZONE {
        CurveTransformZone::TiltHigh
    } else {
        CurveTransformZone::Shift
    }
}

pub(super) fn transform_pointer(response: &egui::Response, plot: Rect) -> Option<Pos2> {
    response
        .hover_pos()
        .or_else(|| response.interact_pointer_pos())
        .filter(|pointer| plot.contains(*pointer))
}

pub(super) fn curve_depth_from_drag(initial: f32, delta_y: f32, frame_height: f32) -> f32 {
    let travel = delta_y / frame_height.max(1.0);
    if travel >= 0.0 {
        initial * 2.0_f32.powf(travel * 3.0)
    } else {
        initial * (1.0 + travel).max(0.0)
    }
}

pub(super) fn inverse_transformed_edit_point(
    pointer: Pos2,
    plot: Rect,
    context: &PlotContext<'_>,
) -> (usize, f32) {
    let output_bin = x_to_active_bin(pointer.x, plot, context.sample_rate, context.fft_size);
    let shift_ratio = 2.0_f32.powf(context.curve_shift_semitones / 12.0);
    let source_bin = (output_bin as f32 / shift_ratio)
        .round()
        .clamp(0.0, (context.fft_size / 2) as f32) as usize;
    let frequency = (output_bin as f32 * context.sample_rate / context.fft_size as f32).max(20.0);
    let desired_db = y_to_curve_db(pointer.y, plot, context.display_range_db);
    let depth = (context.curve_depth_percent * 0.01).max(0.01);
    let tilt = context.curve_tilt_db_per_octave * (frequency / 1000.0).log2();
    (
        source_bin,
        (desired_db / depth - tilt).clamp(MANUAL_CURVE_MUTE_DB, 0.0),
    )
}

pub(super) fn capture_spectrum_to_curve(
    power_sum: &[f64; ANALYZER_POINTS],
    frames: u64,
    curve: &CurveState,
    sample_rate: f32,
    fft_size: usize,
) -> bool {
    if frames == 0 {
        return false;
    }
    let frame_scale = 1.0 / frames as f64;
    let peak = power_sum
        .iter()
        .map(|power| power * frame_scale)
        .fold(0.0_f64, f64::max);
    if peak <= 1.0e-8 {
        return false;
    }
    let max_frequency = display_max_frequency(sample_rate);
    let frequency_ratio = max_frequency / MIN_DISPLAY_FREQUENCY_HZ;
    for active_bin in 0..=fft_size / 2 {
        let frequency = (active_bin as f32 * sample_rate / fft_size as f32)
            .clamp(MIN_DISPLAY_FREQUENCY_HZ, max_frequency);
        let normalized = (frequency / MIN_DISPLAY_FREQUENCY_HZ).ln() / frequency_ratio.ln();
        let position = normalized * (ANALYZER_POINTS - 1) as f32;
        let left = position.floor() as usize;
        let right = (left + 1).min(ANALYZER_POINTS - 1);
        let fraction = (position - left as f32) as f64;
        let power =
            (power_sum[left] + (power_sum[right] - power_sum[left]) * fraction) * frame_scale;
        let db = (10.0 * (power / peak).max(1.0e-6).log10()) as f32;
        let (first, last) = active_bin_master_span(active_bin, fft_size);
        for master_index in first..=last {
            curve.set(master_index, db);
        }
    }
    true
}

pub(super) fn flip_curve(source: &[f32; MANUAL_MASK_POINTS], curve: &CurveState) -> bool {
    if source.iter().any(|db| !db.is_finite()) {
        return false;
    }
    let Some(minimum) = source
        .iter()
        .copied()
        .filter(|db| db.is_finite())
        .reduce(f32::min)
    else {
        return false;
    };
    let Some(maximum) = source
        .iter()
        .copied()
        .filter(|db| db.is_finite())
        .reduce(f32::max)
    else {
        return false;
    };
    if maximum - minimum <= f32::EPSILON {
        return false;
    }

    for (index, db) in source.iter().copied().enumerate() {
        curve.set_transformed(index, minimum + maximum - db);
    }
    true
}

pub(super) fn edit_with_bins(
    curve: &CurveState,
    previous: Option<EditPoint>,
    current: EditPoint,
    fft_size: usize,
) {
    let previous = previous.unwrap_or(current);
    let start = previous.active_bin.min(current.active_bin);
    let end = previous.active_bin.max(current.active_bin);
    for active_bin in start..=end {
        let fraction = if end == start {
            1.0
        } else {
            (active_bin as f32 - previous.active_bin as f32)
                / (current.active_bin as f32 - previous.active_bin as f32)
        };
        let db = previous.db + (current.db - previous.db) * fraction;
        let (first, last) = active_bin_master_span(active_bin, fft_size);
        for master_index in first..=last {
            curve.set(master_index, db);
        }
    }
}

pub(super) fn edit_with_soft_bins(
    curve: &CurveState,
    previous: Option<EditPoint>,
    current: EditPoint,
    fft_size: usize,
    radius_bins: usize,
) {
    let previous = previous.unwrap_or(current);
    let stroke_start = previous.active_bin.min(current.active_bin);
    let stroke_end = previous.active_bin.max(current.active_bin);
    let radius = radius_bins.max(1);
    let first_affected = stroke_start.saturating_sub(radius);
    let last_affected = stroke_end.saturating_add(radius).min(fft_size / 2);
    let mut master_curve = [0.0; MANUAL_MASK_POINTS];
    curve.copy_to(&mut master_curve);
    for active_bin in first_affected..=last_affected {
        let (target_db, distance) = if stroke_start == stroke_end {
            (current.db, active_bin.abs_diff(current.active_bin) as f32)
        } else if active_bin < stroke_start {
            let endpoint_db = if previous.active_bin < current.active_bin {
                previous.db
            } else {
                current.db
            };
            (endpoint_db, (stroke_start - active_bin) as f32)
        } else if active_bin > stroke_end {
            let endpoint_db = if previous.active_bin < current.active_bin {
                current.db
            } else {
                previous.db
            };
            (endpoint_db, (active_bin - stroke_end) as f32)
        } else {
            let fraction = (active_bin as f32 - previous.active_bin as f32)
                / (current.active_bin as f32 - previous.active_bin as f32);
            (previous.db + (current.db - previous.db) * fraction, 0.0)
        };
        let normalized_distance = (distance / radius as f32).clamp(0.0, 1.0);
        let falloff = 0.5 * (1.0 + (PI * normalized_distance).cos());
        let original = master_curve[active_bin_to_master_index(active_bin, fft_size)];
        let db = original + (target_db - original) * falloff;
        let (first, last) = active_bin_master_span(active_bin, fft_size);
        for master_index in first..=last {
            curve.set(master_index, db);
        }
    }
}

pub(super) fn soft_pencil_radius_bins(x: f32, plot: Rect, context: &PlotContext<'_>) -> usize {
    let shift_ratio = 2.0_f32.powf(context.curve_shift_semitones / 12.0);
    let source_bin = |screen_x: f32| {
        (x_to_active_bin(screen_x, plot, context.sample_rate, context.fft_size) as f32
            / shift_ratio)
            .round()
            .clamp(0.0, (context.fft_size / 2) as f32) as usize
    };
    let center = source_bin(x);
    let left = source_bin((x - SOFT_PENCIL_RADIUS_PX).max(plot.left()));
    let right = source_bin((x + SOFT_PENCIL_RADIUS_PX).min(plot.right()));
    center
        .saturating_sub(left)
        .max(right.saturating_sub(center))
        .max(1)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pitch_ruler(
    ui: &mut egui::Ui,
    display: &AnalysisDisplay,
    pinned_notes: &PinnedNotesState,
    palette: Palette,
    drag_value: &mut Option<bool>,
    intro_elapsed_seconds: f32,
    tuning: &crate::mts_client::Tuning,
    tuning_name: &str,
) -> (bool, Option<usize>, Rect) {
    let mut edit_started = false;
    let (outer, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 36.0),
        Sense::click_and_drag(),
    );
    let pixels_per_point = ui.pixels_per_point();
    let pixel = 1.0 / pixels_per_point;
    let mut rect = outer
        .shrink2(Vec2::new(10.0, 0.0))
        .round_to_pixels(pixels_per_point);
    if tuning.active {
        rect.min.y += 11.0;
    }
    let painter = ui.painter_at(outer);
    painter.rect_filled(rect, 0.0, palette.panel);
    let mut octave_labels = Vec::new();
    let mut key_edges = Vec::new();
    let max_frequency = display_max_frequency(display.sample_rate());
    let mut piano_right = rect.left();

    let cells = tuning.cells().unwrap_or_else(|| {
        (0..MIDI_NOTES)
            .map(|note| {
                let center = note_frequency(note as u8);
                let lower = center * 2.0_f32.powf(-1.0 / 24.0);
                let upper = center * 2.0_f32.powf(1.0 / 24.0);
                (note, lower, upper)
            })
            .collect()
    });
    let mut hit_cells = Vec::new();
    for (note, lower, upper) in cells {
        if upper < MIN_DISPLAY_FREQUENCY_HZ || lower > max_frequency {
            continue;
        }
        let left = frequency_to_x(lower, rect, max_frequency);
        let right = frequency_to_x(upper, rect, max_frequency).min(if tuning.active {
            rect.right() - 58.0
        } else {
            rect.right()
        });
        if left >= right {
            continue;
        }
        let tile = Rect::from_min_max(
            Pos2::new(left, rect.top()),
            Pos2::new(right.max(left + pixel), rect.bottom()),
        )
        .round_to_pixels(pixels_per_point);
        piano_right = piano_right.max(tile.right());
        let pitch_class = note % 12;
        let accidental = !tuning.active && matches!(pitch_class, 1 | 3 | 6 | 8 | 10);
        let base = if tuning.active && !tuning.mapped[note] {
            palette.panel
        } else if tuning.active && tuning.boundary(note) {
            palette.octave_key
        } else if accidental {
            palette.accidental
        } else if !tuning.active && pitch_class == 0 {
            palette.octave_key
        } else {
            palette.key
        };
        let pinned = pinned_notes.get(note);
        let level = display
            .note_level(note)
            .max(if pinned { 0.32 } else { 0.0 });
        let ruler_position = ((tile.center().x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        let intro = ruler_intro_strength(ruler_position, intro_elapsed_seconds) * 0.42;
        let fill = mix_color(base, palette.blue, level.max(intro).clamp(0.0, 1.0));
        painter.rect_filled(tile, 0.0, fill);
        if level > 0.002 {
            painter.rect_filled(
                Rect::from_min_size(tile.left_top(), Vec2::new(tile.width(), 2.0 * pixel)),
                0.0,
                with_alpha(
                    palette.blue,
                    (display.note_level(note).clamp(0.0, 1.0) * 255.0) as u8,
                ),
            );
        }
        if pinned {
            painter.circle_filled(
                Pos2::new(tile.center().x, tile.bottom() - 2.0 * pixel),
                pixel,
                palette.blue,
            );
        }
        key_edges.push(tile.left());
        if tuning.mapped[note] {
            hit_cells.push((note, tile));
        }

        if (if tuning.active {
            tuning.boundary(note) || (tuning.map_size.is_none() && note % 12 == 0)
        } else {
            pitch_class == 0
        }) && (tuning.active || tile.width() > 5.0)
        {
            let octave = midi_note_octave(note as i32);
            octave_labels.push((
                Pos2::new(tile.center().x, tile.bottom() - 3.0),
                if tuning.active {
                    format!("{note}")
                } else {
                    format!("C{octave}")
                },
                if level > 0.25 {
                    palette.ink
                } else {
                    palette.muted
                },
            ));
        }
    }
    for edge in key_edges {
        painter.line_segment(
            [Pos2::new(edge, rect.top()), Pos2::new(edge, rect.bottom())],
            Stroke::new(pixel, palette.rule),
        );
    }
    if tuning.active {
        ui.painter_at(outer).text(
            Pos2::new(rect.left(), outer.top()),
            Align2::LEFT_TOP,
            format!(
                "MTS · {}",
                if tuning_name.is_empty() {
                    "tuned notes".to_owned()
                } else {
                    tuning_name.chars().take(64).collect()
                }
            ),
            FontId::monospace(oiko_ui::typography::TEXT_SMALL),
            palette.blue,
        );
    }
    for (position, label, color) in octave_labels {
        painter.text(
            position,
            Align2::CENTER_BOTTOM,
            label,
            FontId::monospace(oiko_ui::typography::TEXT_SMALL),
            color,
        );
    }
    if tuning.active {
        piano_right = rect.right() - 58.0;
    }
    let reset_width = rect.right() - piano_right - 10.0;
    let mut reset_hovered = false;
    let mut hold_hovered = false;
    if reset_width >= 38.0 {
        let button_left = piano_right + 6.0;
        let hold_rect = Rect::from_min_size(
            Pos2::new(button_left, outer.top() + 2.0),
            Vec2::new(reset_width, 15.0),
        );
        let hold = ui.interact(hold_rect, Id::new("hold-incoming-notes"), Sense::click());
        hold_hovered = hold.hovered();
        if hold.clicked() {
            edit_started = true;
            pinned_notes.toggle_capture_incoming();
        }
        let holding = pinned_notes.capture_incoming();
        painter.rect_filled(
            hold_rect,
            3.0,
            if holding {
                mix_color(palette.panel, palette.blue, 0.24)
            } else if hold.hovered() {
                palette.track
            } else {
                palette.panel
            },
        );
        painter.rect_stroke(
            hold_rect,
            3.0,
            Stroke::new(1.0, if holding { palette.blue } else { palette.rule }),
            StrokeKind::Inside,
        );
        painter.text(
            hold_rect.center(),
            Align2::CENTER_CENTER,
            "HOLD",
            FontId::monospace(oiko_ui::typography::TEXT_SMALL),
            if holding || hold.hovered() {
                palette.blue
            } else {
                palette.muted
            },
        );

        let reset_rect = Rect::from_min_size(
            Pos2::new(button_left, outer.top() + 19.0),
            Vec2::new(reset_width, 15.0),
        );
        let reset = ui.interact(reset_rect, Id::new("reset-pinned-notes"), Sense::click());
        reset_hovered = reset.hovered();
        if reset.clicked() {
            edit_started = (0..MIDI_NOTES).any(|note| pinned_notes.get(note));
            pinned_notes.clear();
        }
        painter.rect_filled(
            reset_rect,
            3.0,
            if reset.hovered() {
                palette.track
            } else {
                palette.panel
            },
        );
        painter.rect_stroke(
            reset_rect,
            3.0,
            Stroke::new(1.0, palette.rule),
            StrokeKind::Inside,
        );
        painter.text(
            reset_rect.center(),
            Align2::CENTER_CENTER,
            "RESET",
            FontId::monospace(oiko_ui::typography::TEXT_SMALL),
            if reset.hovered() {
                palette.blue
            } else {
                palette.muted
            },
        );
    }

    let pointer_note = response.interact_pointer_pos().and_then(|pointer| {
        hit_cells
            .iter()
            .find(|(_, tile)| tile.contains(pointer))
            .map(|(note, _)| *note)
    });
    if response.clicked()
        && let Some(note) = pointer_note
    {
        edit_started = true;
        pinned_notes.set_from_ui(note, !pinned_notes.get(note));
    } else if response.dragged()
        && let Some(note) = pointer_note
    {
        edit_started = drag_value.is_none();
        let value = *drag_value.get_or_insert_with(|| !pinned_notes.get(note));
        pinned_notes.set_from_ui(note, value);
    }
    if response.drag_stopped() {
        *drag_value = None;
    }
    if response.hovered() || reset_hovered || hold_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let hovered_note = response
        .hover_pos()
        .filter(|p| rect.contains(*p) && p.x <= piano_right)
        .and_then(|p| {
            hit_cells
                .iter()
                .find(|(_, tile)| tile.contains(p))
                .map(|(note, _)| *note)
        });
    (edit_started, hovered_note, outer)
}
