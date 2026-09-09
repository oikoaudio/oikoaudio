//! Host parameter controls, header and spectrum range menu.
use super::parameter_history::TrackedParamSetter;
use super::rendering::{mix_color, with_alpha};
use super::request_settled_scale;
use crate::parameters::{MotionRateDivision, SpectralParams};
use egui::{
    Align2, Color32, FontId, Id, Key, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Vec2,
};
use nice_plug::params::{BoolParam, EnumParam, FloatParam, Param};
use oiko_plugin::gestures::ParameterWriter;
use oiko_plugin::parameter_controls::{DragMode, parameter_drag};
use oiko_ui::theme::{Palette, apply_theme};
use std::sync::atomic::{AtomicBool, Ordering};

const PROJECT_URL: &str = "https://oikoaudio.com/weft/";
pub(super) const CONTROL_GAP: f32 = 20.0;
pub(super) fn fixed_parameter<P: Param>(
    ui: &mut egui::Ui,
    width: f32,
    style: (bool, Color32),
    label: &str,
    param: &P,
    setter: &TrackedParamSetter<'_>,
    _help: &str,
) {
    let (enabled, accent) = style;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("fixed-parameter", label))
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect);
    child.set_width(width);
    child.set_max_width(width);
    child.add_enabled_ui(enabled, |ui| {
        ui.label(
            egui::RichText::new(label)
                .small()
                .strong()
                .color(ui.visuals().weak_text_color()),
        );
        compact_numeric_value(
            ui,
            Vec2::new(width, 18.0),
            accent,
            label,
            None,
            None,
            param,
            setter,
        );
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fixed_motion_rate_parameter(
    ui: &mut egui::Ui,
    width: f32,
    style: (bool, Color32),
    free_rate: &FloatParam,
    sync: &BoolParam,
    division: &EnumParam<MotionRateDivision>,
    tempo_bpm: f32,
    sample_rate: f32,
    fft_size: usize,
    setter: &TrackedParamSetter<'_>,
    help: &str,
) {
    let (enabled, accent) = style;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("fixed-motion-rate")
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect);
    child.set_width(width);
    child.set_max_width(width);
    child.add_enabled_ui(enabled, |ui| {
        let palette = Palette::new(ui.visuals().dark_mode);
        let (header_rect, _) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::hover());
        ui.painter().text(
            header_rect.left_center(),
            Align2::LEFT_CENTER,
            "RATE",
            FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            ui.visuals().weak_text_color(),
        );
        let rate_ceiling = crate::maximum_motion_rate_hz(sample_rate, fft_size);
        if rate_ceiling < crate::parameters::MAX_FREE_MOTION_RATE_HZ - 0.01 {
            ui.painter().text(
                Pos2::new(header_rect.left() + 30.0, header_rect.center().y),
                Align2::LEFT_CENTER,
                format!("≤{rate_ceiling:.1} Hz"),
                FontId::proportional(oiko_ui::typography::TEXT_SMALL),
                palette.muted,
            );
        }
        let mode_rect = Rect::from_min_size(
            Pos2::new(header_rect.right() - 58.0, header_rect.top()),
            Vec2::new(58.0, 14.0),
        );
        let free_rect = Rect::from_min_max(
            mode_rect.left_top(),
            Pos2::new(mode_rect.center().x, mode_rect.bottom()),
        );
        let sync_rect = Rect::from_min_max(
            Pos2::new(mode_rect.center().x, mode_rect.top()),
            mode_rect.right_bottom(),
        );
        let free_choice = ui.interact(
            free_rect,
            Id::new("motion-rate-free-choice"),
            Sense::click(),
        );
        let sync_choice = ui.interact(
            sync_rect,
            Id::new("motion-rate-sync-choice"),
            Sense::click(),
        );
        ui.painter().rect_filled(mode_rect, 2.0, palette.field);
        let selected_rect = if sync.value() { sync_rect } else { free_rect };
        ui.painter()
            .rect_filled(selected_rect.shrink(1.0), 1.0, with_alpha(accent, 24));
        if free_choice.hovered() {
            ui.painter()
                .rect_filled(free_rect.shrink(1.0), 1.0, palette.track);
        }
        if sync_choice.hovered() {
            ui.painter()
                .rect_filled(sync_rect.shrink(1.0), 1.0, palette.track);
        }
        ui.painter().rect_stroke(
            mode_rect,
            2.0,
            Stroke::new(1.0, palette.rule),
            StrokeKind::Inside,
        );
        ui.painter().line_segment(
            [
                Pos2::new(selected_rect.left() + 3.0, selected_rect.bottom() - 1.0),
                Pos2::new(selected_rect.right() - 3.0, selected_rect.bottom() - 1.0),
            ],
            Stroke::new(1.0, accent),
        );
        if free_choice.hovered() || sync_choice.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        ui.painter().text(
            free_rect.center(),
            Align2::CENTER_CENTER,
            "FREE",
            FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            if !sync.value() {
                accent
            } else if free_choice.hovered() {
                palette.ink
            } else {
                palette.muted
            },
        );
        ui.painter().text(
            sync_rect.center(),
            Align2::CENTER_CENTER,
            "SYNC",
            FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            if sync.value() {
                accent
            } else if sync_choice.hovered() {
                palette.ink
            } else {
                palette.muted
            },
        );
        let requested_mode = if free_choice.clicked() {
            Some(false)
        } else if sync_choice.clicked() {
            Some(true)
        } else {
            None
        };
        if let Some(next_sync) = requested_mode.filter(|next_sync| *next_sync != sync.value()) {
            if next_sync {
                let closest = MotionRateDivision::closest_to_hz(free_rate.value(), tempo_bpm);
                setter.begin_set_parameter(division);
                setter.set_parameter(division, closest);
                setter.end_set_parameter(division);
            } else {
                let equivalent_rate = division
                    .value()
                    .rate_hz(tempo_bpm)
                    .clamp(0.01, crate::parameters::MAX_FREE_MOTION_RATE_HZ);
                setter.begin_set_parameter(free_rate);
                setter.set_parameter(free_rate, equivalent_rate);
                setter.end_set_parameter(free_rate);
            }
            setter.begin_set_parameter(sync);
            setter.set_parameter(sync, next_sync);
            setter.end_set_parameter(sync);
        }

        if sync.value() {
            step_value(
                ui,
                Vec2::new(width, 18.0),
                "Motion Division",
                division,
                setter,
            )
            .on_hover_text(help);
        } else {
            compact_numeric_value(
                ui,
                Vec2::new(width, 18.0),
                accent,
                "Motion Rate",
                None,
                None,
                free_rate,
                setter,
            )
            .on_hover_text(help);
        }
    });
}

pub(super) fn fixed_motion_phase_size_parameter(
    ui: &mut egui::Ui,
    width: f32,
    style: (bool, Color32),
    phase: &FloatParam,
    size: &FloatParam,
    setter: &TrackedParamSetter<'_>,
) {
    let (enabled, accent) = style;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("fixed-motion-phase-size")
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect);
    child.set_width(width);
    child.set_max_width(width);
    child.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let field_width = (width - 10.0) * 0.5;
            ui.vertical(|ui| {
                ui.set_width(field_width);
                ui.label(
                    egui::RichText::new("PHASE")
                        .small()
                        .strong()
                        .color(ui.visuals().weak_text_color()),
                );
                compact_numeric_value(
                    ui,
                    Vec2::new(field_width, 18.0),
                    accent,
                    "Motion Phase",
                    None,
                    None,
                    phase,
                    setter,
                ).on_hover_text("Offset the repeating motion reference. For particles, shifts birth opportunities while note-on triggers keep their timing.");
            });
            ui.vertical(|ui| {
                ui.set_width(field_width);
                ui.label(
                    egui::RichText::new("SIZE")
                        .small()
                        .strong()
                        .color(ui.visuals().weak_text_color()),
                );
                compact_numeric_value(
                    ui,
                    Vec2::new(field_width, 18.0),
                    accent,
                    "Motion Size",
                    None,
                    None,
                    size,
                    setter,
                ).on_hover_text("Spectral width in octaves. Larger values broaden particle windows and lengthen Cloud envelopes. Sprinkle timing comes from Note Control Attack and Release; Partials and Rolloff shape its harmonic choices.");
            });
        });
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fixed_envelope_parameter(
    ui: &mut egui::Ui,
    width: f32,
    style: (bool, Color32),
    label: &str,
    attack: &FloatParam,
    release: &FloatParam,
    intrinsic_transition_ms: f32,
    setter: &TrackedParamSetter<'_>,
    help: &str,
) {
    let (enabled, accent) = style;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("fixed-envelope-parameter", label))
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect);
    child.set_width(width);
    child.set_max_width(width);
    child
        .add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let field_width = (width - 10.0) * 0.5;
                ui.vertical(|ui| {
                    ui.set_width(field_width);
                    ui.label(
                        egui::RichText::new("ATTACK")
                            .small()
                            .strong()
                            .color(ui.visuals().weak_text_color()),
                    );
                    compact_numeric_value(
                        ui,
                        Vec2::new(field_width, 18.0),
                        accent,
                        "Note Attack",
                        None,
                        Some(format_effective_note_time(
                            intrinsic_transition_ms + attack.value(),
                        )),
                        attack,
                        setter,
                    );
                });
                ui.vertical(|ui| {
                    ui.set_width(field_width);
                    ui.label(
                        egui::RichText::new("RELEASE")
                            .small()
                            .strong()
                            .color(ui.visuals().weak_text_color()),
                    );
                    compact_numeric_value(
                        ui,
                        Vec2::new(field_width, 18.0),
                        accent,
                        "Note Release",
                        None,
                        Some(format_effective_note_time(
                            intrinsic_transition_ms + release.value(),
                        )),
                        release,
                        setter,
                    );
                });
            });
        })
        .response
        .on_hover_text(help);
}

pub(super) fn format_effective_note_time(milliseconds: f32) -> String {
    if milliseconds >= 1000.0 {
        format!("{:.1}s", milliseconds / 1000.0)
    } else {
        format!("{milliseconds:.0}ms")
    }
}

pub(super) fn estimated_note_transition_ms(fft_size: usize, sample_rate: f32) -> f32 {
    1500.0 * fft_size as f32 / (4.0 * sample_rate.max(1.0))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compact_numeric_value<P: Param>(
    ui: &mut egui::Ui,
    size: Vec2,
    accent: Color32,
    _label: &str,
    prefix: Option<&str>,
    display_override: Option<String>,
    param: &P,
    setter: &TrackedParamSetter<'_>,
) -> egui::Response {
    let palette = Palette::new(ui.visuals().dark_mode);
    let field_id = Id::new(("compact-numeric", param.name()));
    let edit_id = field_id.with("edit");
    let buffer_id = field_id.with("buffer");
    let editing = ui.memory(|memory| memory.has_focus(edit_id));

    if editing {
        let mut buffer = ui
            .data(|data| data.get_temp::<String>(buffer_id))
            .unwrap_or_else(|| param.to_string());
        let response = ui.add_sized(
            size,
            TextEdit::singleline(&mut buffer)
                .id(edit_id)
                .font(egui::TextStyle::Monospace)
                .horizontal_align(egui::Align::Center),
        );
        ui.data_mut(|data| data.insert_temp(buffer_id, buffer.clone()));
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            ui.memory_mut(|memory| memory.surrender_focus(edit_id));
        } else if ui.input(|input| input.key_pressed(Key::Enter)) {
            if let Some(normalized) = param.string_to_normalized_value(&buffer) {
                setter.set_discrete_parameter(param, param.preview_plain(normalized));
            }
            ui.memory_mut(|memory| memory.surrender_focus(edit_id));
        }
        return response;
    }

    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let response = ui.interact(rect, field_id, Sense::click_and_drag());
    let enabled = ui.is_enabled();
    let hovered = enabled && (response.hovered() || response.dragged());
    ui.painter().rect_filled(
        rect,
        2.0,
        if hovered {
            palette.track
        } else {
            palette.field
        },
    );
    let normalized = param.unmodulated_normalized_value().clamp(0.0, 1.0);
    let progress = Rect::from_min_max(
        Pos2::new(rect.left(), rect.bottom() - 2.0),
        Pos2::new(rect.left() + rect.width() * normalized, rect.bottom()),
    );
    ui.painter().rect_filled(
        progress,
        0.0,
        if enabled {
            accent
        } else {
            with_alpha(accent, 45)
        },
    );
    let value = display_override.unwrap_or_else(|| {
        prefix
            .map(|prefix| format!("{prefix} {param}"))
            .unwrap_or_else(|| param.to_string())
    });
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        value,
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        if enabled {
            palette.ink
        } else {
            with_alpha(palette.muted, 90)
        },
    );

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    parameter_drag(
        ui,
        &response,
        param,
        setter,
        DragMode::Absolute {
            range: rect,
            fine_sensitivity: Some(0.0015),
        },
    );
    if response.double_clicked() || ui.input(|input| input.modifiers.command) && response.clicked()
    {
        setter.set_discrete_parameter(param, param.default_plain_value());
    } else if response.clicked() && ui.input(|input| input.modifiers.alt) {
        ui.data_mut(|data| data.insert_temp(buffer_id, param.to_string()));
        ui.memory_mut(|memory| memory.request_focus(edit_id));
    } else if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let next = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        setter.set_discrete_parameter(param, param.preview_plain(next));
    }
    response
}

pub(super) fn fixed_step_parameter<P: Param>(
    ui: &mut egui::Ui,
    width: f32,
    enabled: bool,
    label: &str,
    param: &P,
    setter: &TrackedParamSetter<'_>,
    help: &str,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("fixed-step-parameter", label))
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect);
    child.set_width(width);
    child.set_max_width(width);
    child.add_enabled_ui(enabled, |ui| {
        ui.label(
            egui::RichText::new(label)
                .small()
                .strong()
                .color(ui.visuals().weak_text_color()),
        );
        step_value(ui, Vec2::new(width, 18.0), label, param, setter).on_hover_text(help);
    });
}

pub(super) fn step_value<P: Param>(
    ui: &mut egui::Ui,
    size: Vec2,
    label: &str,
    param: &P,
    setter: &TrackedParamSetter<'_>,
) -> egui::Response {
    let palette = Palette::new(ui.visuals().dark_mode);
    let enabled = ui.is_enabled();
    let ink = if enabled {
        palette.ink
    } else {
        with_alpha(palette.muted, 90)
    };
    let muted = if enabled {
        palette.muted
    } else {
        with_alpha(palette.muted, 70)
    };
    let (rect, whole) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let left_button = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(rect.left() + 20.0, rect.bottom()),
    );
    let right_button = Rect::from_min_max(
        Pos2::new(rect.right() - 20.0, rect.top()),
        rect.right_bottom(),
    );
    let previous = ui.interact(
        left_button,
        Id::new(("step-value-prev", label)),
        Sense::click(),
    );
    let next = ui.interact(
        right_button,
        Id::new(("step-value-next", label)),
        Sense::click(),
    );
    let normalized = param.unmodulated_normalized_value();
    let can_go_previous = normalized > 1.0e-6;
    let can_go_next = normalized < 1.0 - 1.0e-6;
    let hovered = whole.hovered() || previous.hovered() || next.hovered();
    ui.painter().rect_filled(
        rect,
        2.0,
        if hovered {
            palette.track
        } else {
            palette.field
        },
    );
    let track = Rect::from_min_max(
        Pos2::new(left_button.right(), rect.bottom() - 2.0),
        Pos2::new(right_button.left(), rect.bottom()),
    );
    ui.painter().rect_filled(track, 0.0, palette.rule);
    let progress = Rect::from_min_max(
        track.left_top(),
        Pos2::new(
            track.left() + track.width() * normalized.clamp(0.0, 1.0),
            track.bottom(),
        ),
    );
    ui.painter().rect_filled(progress, 0.0, palette.orange);
    ui.painter().text(
        Pos2::new(rect.left() + 7.0, rect.center().y),
        Align2::LEFT_CENTER,
        "‹",
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        if !can_go_previous {
            palette.rule
        } else if previous.hovered() {
            ink
        } else {
            muted
        },
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        param.to_string(),
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        ink,
    );
    ui.painter().text(
        Pos2::new(rect.right() - 7.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "›",
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        if !can_go_next {
            palette.rule
        } else if next.hovered() {
            ink
        } else {
            muted
        },
    );
    if hovered && (can_go_previous || can_go_next) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    parameter_drag(
        ui,
        &whole,
        param,
        setter,
        DragMode::Absolute {
            range: track,
            fine_sensitivity: None,
        },
    );
    if whole.double_clicked() {
        setter.set_discrete_parameter(param, param.default_plain_value());
    } else if !whole.dragged()
        && can_go_next
        && (next.clicked() || (whole.clicked() && !previous.clicked()))
    {
        setter.set_discrete_parameter(
            param,
            param.next_step(param.unmodulated_plain_value(), false),
        );
    } else if !whole.dragged()
        && can_go_previous
        && (previous.clicked() || whole.secondary_clicked())
    {
        setter.set_discrete_parameter(
            param,
            param.previous_step(param.unmodulated_plain_value(), false),
        );
    }
    whole
}

pub(super) fn global_footer(
    ui: &mut egui::Ui,
    palette: Palette,
    params: &SpectralParams,
    setter: &TrackedParamSetter<'_>,
    output_peak: f32,
) {
    let rect = Rect::from_min_max(
        Pos2::new(ui.max_rect().left(), ui.max_rect().bottom() - 30.0),
        ui.max_rect().right_bottom(),
    );
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, palette.rule),
    );

    let height = 20.0;
    let inset = 10.0;
    let gap = CONTROL_GAP;
    let control_width = (rect.width() - inset * 2.0 - gap * 4.0) / 5.0;
    let column_x = |index: usize| rect.left() + inset + index as f32 * (control_width + gap);
    let control_rect = |column: usize| {
        Rect::from_min_max(
            Pos2::new(column_x(column), rect.center().y - height * 0.5 + 1.0),
            Pos2::new(
                column_x(column) + control_width,
                rect.center().y + height * 0.5 - 1.0,
            ),
        )
    };
    let resolution_rect = control_rect(0);
    let velocity_rect = control_rect(1);
    let bend_rect = control_rect(2);
    let smooth_rect = control_rect(3);
    let output_rect = control_rect(4);

    footer_inline_step_param(
        ui,
        resolution_rect,
        "RESOLUTION",
        &params.quality,
        setter,
        palette,
    );

    let mut velocity_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("footer-velocity-sensitivity")
            .max_rect(velocity_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    velocity_ui.set_clip_rect(velocity_rect);
    compact_numeric_value(
        &mut velocity_ui,
        velocity_rect.size(),
        palette.blue,
        "Velocity Sensitivity",
        Some("VEL SENS"),
        None,
        &params.velocity_sensitivity_percent,
        setter,
    );

    footer_toggle(
        ui,
        smooth_rect,
        "SMOOTH",
        &params.smooth_spectral,
        setter,
        palette,
    );

    let mut bend_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("footer-pitch-bend-range")
            .max_rect(bend_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    bend_ui.set_clip_rect(bend_rect);
    compact_numeric_value(
        &mut bend_ui,
        bend_rect.size(),
        palette.blue,
        "Pitch Bend Range",
        Some("BEND ±"),
        None,
        &params.pitch_bend_range,
        setter,
    );

    let mut output_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("footer-output")
            .max_rect(output_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    output_ui.set_clip_rect(output_rect);
    compact_numeric_value(
        &mut output_ui,
        output_rect.size(),
        palette.blue,
        "Output",
        Some("OUTPUT"),
        None,
        &params.output_gain_db,
        setter,
    );
    let meter_db = 20.0 * output_peak.max(1e-6).log10();
    let meter_fraction = ((meter_db + 60.0) / 66.0).clamp(0.0, 1.0);
    let meter = Rect::from_min_size(
        output_rect.left_top(),
        Vec2::new(output_rect.width() * meter_fraction, 2.0),
    );
    ui.painter().rect_filled(
        meter,
        0.0,
        if output_peak >= 1.0 {
            palette.orange
        } else {
            palette.blue
        },
    );
}

pub(super) fn footer_toggle(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    param: &BoolParam,
    setter: &TrackedParamSetter<'_>,
    palette: Palette,
) {
    let response = ui.interact(rect, Id::new(("footer-toggle", label)), Sense::click());
    ui.painter().rect_filled(
        rect,
        2.0,
        if response.hovered() {
            palette.track
        } else {
            palette.field
        },
    );
    let enabled = param.value();
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{label} {}", if enabled { "ON" } else { "OFF" }),
        FontId::monospace(oiko_ui::typography::TEXT_SMALL),
        if enabled { palette.blue } else { palette.ink },
    );
    if enabled {
        ui.painter().line_segment(
            [
                Pos2::new(rect.left() + 4.0, rect.bottom() - 1.0),
                Pos2::new(rect.right() - 4.0, rect.bottom() - 1.0),
            ],
            Stroke::new(1.0, palette.blue),
        );
    }
    if response.clicked() {
        setter.set_discrete_parameter(param, !enabled);
    }
    response.on_hover_text(
        "A/B the current Hann path against a Blackman window, softened spectral edges, and short mask smoothing.",
    );
}

pub(super) fn footer_inline_step_param<P: Param>(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    param: &P,
    setter: &TrackedParamSetter<'_>,
    palette: Palette,
) {
    let left_button = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(rect.left() + 20.0, rect.bottom()),
    );
    let right_button = Rect::from_min_max(
        Pos2::new(rect.right() - 20.0, rect.top()),
        rect.right_bottom(),
    );
    let previous = ui.interact(
        left_button,
        Id::new(("footer-param-prev", label)),
        Sense::click(),
    );
    let next = ui.interact(
        right_button,
        Id::new(("footer-param-next", label)),
        Sense::click(),
    );
    let normalized = param.unmodulated_normalized_value();
    let can_go_previous = normalized > 1.0e-6;
    let can_go_next = normalized < 1.0 - 1.0e-6;
    ui.painter().rect_filled(rect, 2.0, palette.field);
    if previous.hovered() && can_go_previous {
        ui.painter().rect_filled(left_button, 2.0, palette.track);
    }
    if next.hovered() && can_go_next {
        ui.painter().rect_filled(right_button, 2.0, palette.track);
    }
    ui.painter().text(
        left_button.center(),
        Align2::CENTER_CENTER,
        "‹",
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        if !can_go_previous {
            palette.rule
        } else if previous.hovered() {
            palette.ink
        } else {
            palette.muted
        },
    );
    ui.painter().text(
        right_button.center(),
        Align2::CENTER_CENTER,
        "›",
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        if !can_go_next {
            palette.rule
        } else if next.hovered() {
            palette.ink
        } else {
            palette.muted
        },
    );
    let mut label_job = egui::text::LayoutJob::default();
    label_job.append(
        "MODE",
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            color: palette.muted,
            ..Default::default()
        },
    );
    label_job.append(
        "  ",
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            color: palette.muted,
            ..Default::default()
        },
    );
    label_job.append(
        &param.to_string().to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(oiko_ui::typography::TEXT_SMALL),
            color: palette.ink,
            ..Default::default()
        },
    );
    let label = ui.painter().layout_job(label_job);
    ui.painter()
        .galley(rect.center() - label.size() * 0.5, label, palette.ink);
    if (previous.hovered() && can_go_previous) || (next.hovered() && can_go_next) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if can_go_next && next.clicked() {
        setter.set_discrete_parameter(
            param,
            param.next_step(param.unmodulated_plain_value(), false),
        );
    } else if can_go_previous && previous.clicked() {
        setter.set_discrete_parameter(
            param,
            param.previous_step(param.unmodulated_plain_value(), false),
        );
    }
}

pub(super) fn header(
    ui: &mut egui::Ui,
    dark: &AtomicBool,
    about_open: &mut bool,
    ui_scale: &crate::state::UiScaleState,
) {
    let mut requested_dark = dark.load(Ordering::Relaxed);
    let header = oiko_ui::chrome::header(ui, "WEFT", &mut requested_dark);
    if header.theme_changed {
        dark.store(requested_dark, Ordering::Relaxed);
        apply_theme(ui.ctx(), requested_dark);
    }
    let response = oiko_ui::chrome::about_menu(
        &header,
        oiko_ui::chrome::ProductInfo {
            name: "Oiko Weft",
            version: env!("CARGO_PKG_VERSION"),
            website: PROJECT_URL,
        },
        ui_scale.get(),
        |_| {},
    );
    *about_open = response.is_open;
    if let Some(scale) = response.scale {
        ui_scale.set(scale);
        request_settled_scale(ui.ctx(), scale);
    }
}

pub(super) fn spectrum_range_selector(
    ui: &mut egui::Ui,
    position: Pos2,
    palette: Palette,
    spectrum_range: &crate::state::SpectrumRangeState,
    menu_open: &mut bool,
) -> bool {
    egui::Area::new(Id::new("spectrum-range-area"))
        .order(egui::Order::Foreground)
        .fixed_pos(position)
        .movable(false)
        .show(ui.ctx(), |area_ui| {
            area_ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (button_rect, button) =
                area_ui.allocate_exact_size(Vec2::new(48.0, 12.0), Sense::click());
            if button.clicked() {
                *menu_open = !*menu_open;
            }
            area_ui.painter().rect_filled(
                button_rect,
                1.5,
                if button.hovered() || *menu_open {
                    palette.track
                } else {
                    palette.field
                },
            );
            area_ui.painter().rect_stroke(
                button_rect,
                1.5,
                Stroke::new(1.0, palette.rule),
                StrokeKind::Inside,
            );
            area_ui.painter().text(
                button_rect.center(),
                Align2::CENTER_CENTER,
                format!("{:.0} dB ▾", spectrum_range.get()),
                FontId::monospace(oiko_ui::typography::TEXT_SMALL),
                if button.hovered() || *menu_open {
                    palette.ink
                } else {
                    palette.muted
                },
            );

            let mut menu_hovered = false;
            if *menu_open {
                for value in [30.0_f32, 60.0, 90.0, 144.0] {
                    let (option_rect, option) =
                        area_ui.allocate_exact_size(Vec2::new(48.0, 13.0), Sense::click());
                    menu_hovered |= option.hovered();
                    let selected = (spectrum_range.get() - value).abs() < 0.1;
                    area_ui.painter().rect_filled(
                        option_rect,
                        0.0,
                        if selected {
                            mix_color(palette.field, palette.orange, 0.12)
                        } else if option.hovered() {
                            palette.track
                        } else {
                            palette.panel
                        },
                    );
                    area_ui.painter().rect_stroke(
                        option_rect,
                        0.0,
                        Stroke::new(1.0, palette.rule),
                        StrokeKind::Inside,
                    );
                    area_ui.painter().text(
                        option_rect.center(),
                        Align2::CENTER_CENTER,
                        format!("{value:.0} dB"),
                        FontId::monospace(oiko_ui::typography::TEXT_SMALL),
                        if selected {
                            palette.orange
                        } else if option.hovered() {
                            palette.ink
                        } else {
                            palette.muted
                        },
                    );
                    if option.clicked() {
                        spectrum_range.set(value);
                        *menu_open = false;
                    }
                }
            }
            if button.hovered() || menu_hovered {
                area_ui
                    .ctx()
                    .set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            button.hovered() || menu_hovered
        })
        .inner
}
