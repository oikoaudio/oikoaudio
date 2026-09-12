mod phase;
use crate::display_data::ModulationDisplay;
use oiko_plugin::{
    gestures::ParameterWriter,
    parameter_controls::{DragMode, parameter_drag, parameter_drag_mapped},
};
use std::{
    f32::consts::PI,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use egui::{
    Align2, Color32, FontId, Frame, Id, LayerId, Order, Pos2, Rect, Response, Sense, Shape, Stroke,
    UiBuilder, Vec2,
};
use nice_plug::{context::gui::GuiContext, params::Param};
use nice_plug_egui::{NiceEguiApp, baseview::HandlerError};

use crate::WowParams;

pub(crate) const EDITOR_WIDTH: f32 = 500.0;
pub(crate) const EDITOR_HEIGHT: f32 = 390.0;
pub(crate) use oiko_ui::scale::closest_ui_scale;
const PROJECT_URL: &str = "https://oikoaudio.com/wow/";

pub struct WowEditor {
    params: Arc<WowParams>,
    display: Arc<ModulationDisplay>,
    gui_context: Option<GuiContext>,
    dark: Arc<AtomicBool>,
    about_open: bool,
    gestures: oiko_plugin::gestures::ParameterGestures,
}

impl WowEditor {
    pub(crate) fn new(params: Arc<WowParams>, display: Arc<ModulationDisplay>) -> Self {
        Self {
            params,
            display,
            gui_context: None,
            dark: Arc::new(AtomicBool::new(true)),
            about_open: false,
            gestures: Default::default(),
        }
    }
}

impl NiceEguiApp for WowEditor {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), HandlerError> {
        self.gui_context = Some(nice_gui_ctx);
        apply_theme(&egui_ctx, self.dark.load(Ordering::Relaxed));
        let settled_scale = closest_ui_scale(self.params.ui_scale.get());
        self.params.ui_scale.set(settled_scale);
        oiko_ui::scale::initialize_scale(
            &egui_ctx,
            settled_scale,
            Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT),
        );
        Ok(())
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut nice_plug_egui::Frame) {
        self.draw_ui(root_ui);
    }

    fn editor_closed(&mut self) {
        if let Some(context) = &self.gui_context {
            self.gestures.finish(&*self.params, &context.param_setter());
        }
        self.gui_context = None;
    }
}

impl WowEditor {
    fn draw_ui(&mut self, root_ui: &mut egui::Ui) {
        root_ui
            .ctx()
            .request_repaint_after(Duration::from_millis(16));
        let dark = self.dark.load(Ordering::Relaxed);
        let palette = Palette::new(dark);
        let setter = self
            .gui_context
            .as_ref()
            .expect("the GUI context is set before drawing")
            .param_setter();

        let tracked = self.gestures.setter(&setter);
        let setter = &tracked;

        root_ui
            .painter()
            .rect_filled(root_ui.max_rect(), 0.0, palette.panel);

        let render_scale = oiko_ui::scale::canvas_scale(self.params.ui_scale.get());
        let transform = egui::emath::TSTransform::from_scaling(render_scale);
        let content_layer = LayerId::new(Order::Middle, Id::new("wow-scaled-content"));
        root_ui.ctx().set_transform_layer(content_layer, transform);

        let content_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT));
        let mut content_ui = root_ui.new_child(
            UiBuilder::new()
                .id_salt("wow-scaled-content")
                .layer_id(content_layer)
                .max_rect(content_rect)
                .layout(*root_ui.layout()),
        );
        content_ui.set_clip_rect(content_rect);
        let ui = &mut content_ui;
        ui.set_min_size(Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT));
        ui.painter().rect_filled(ui.max_rect(), 0.0, palette.panel);

        header(ui, &self.dark, &mut self.about_open, &self.params.ui_scale);
        motion_display(
            ui,
            palette,
            &self.display,
            crate::display_rate_scale(&self.params, self.display.tempo()),
        );

        Frame::NONE
            .inner_margin(egui::Margin::symmetric(8, 15))
            .show(ui, |ui| {
                ui.columns(4, |columns| {
                    rate_knob(
                        &mut columns[0],
                        "WOW RATE",
                        RateControl {
                            free: &self.params.rate,
                            sync: &self.params.rate_sync,
                            division: &self.params.rate_division,
                            range: crate::rate_sync::WOW,
                        },
                        self.display.tempo(),
                        setter,
                        palette,
                    );
                    rate_knob(
                        &mut columns[1],
                        "FLUTTER RATE",
                        RateControl {
                            free: &self.params.flutter_rate,
                            sync: &self.params.flutter_rate_sync,
                            division: &self.params.flutter_rate_division,
                            range: crate::rate_sync::FLUTTER,
                        },
                        self.display.tempo(),
                        setter,
                        palette,
                    );
                    phase::control(&mut columns[2], &self.params, setter, palette);
                    knob(
                        &mut columns[3],
                        "AMOUNT",
                        &self.params.amount,
                        setter,
                        palette,
                    );
                });

                ui.add_space(8.0);
                ui.columns(2, |columns| {
                    Frame::NONE
                        .inner_margin(egui::Margin {
                            left: 0,
                            right: 14,
                            top: 0,
                            bottom: 0,
                        })
                        .show(&mut columns[0], |ui| {
                            parameter_slider(
                                ui,
                                "DRIFT",
                                &self.params.drift,
                                setter,
                                palette,
                                false,
                            );
                        });
                    Frame::NONE
                        .inner_margin(egui::Margin {
                            left: 14,
                            right: 0,
                            top: 0,
                            bottom: 0,
                        })
                        .show(&mut columns[1], |ui| {
                            parameter_slider(
                                ui,
                                "WOW / FLUTTER",
                                &self.params.wow_flutter,
                                setter,
                                palette,
                                true,
                            );
                        });
                });
            });

        footer(ui, palette, &self.params, setter);
        if !ui.input(|input| input.pointer.any_down()) || !ui.input(|input| input.focused) {
            let setter = self.gui_context.as_ref().unwrap().param_setter();
            self.gestures.finish(&*self.params, &setter);
        }
        if let Some(scale) = oiko_ui::resize_grip::show(
            root_ui,
            Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT),
            self.params.ui_scale.get(),
        ) {
            self.params.ui_scale.set(scale);
            request_settled_scale(root_ui.ctx(), scale);
        }
    }
}

fn request_settled_scale(context: &egui::Context, scale: f32) {
    oiko_ui::scale::request_scale(context, scale, Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT));
}

use oiko_ui::theme::{Palette, apply_theme};

fn header(
    ui: &mut egui::Ui,
    dark: &AtomicBool,
    about_open: &mut bool,
    ui_scale: &crate::UiScaleState,
) {
    let mut requested_dark = dark.load(Ordering::Relaxed);
    let header = oiko_ui::chrome::header(ui, "WOW", &mut requested_dark);
    if header.theme_changed {
        dark.store(requested_dark, Ordering::Relaxed);
        apply_theme(ui.ctx(), requested_dark);
    }
    let response = oiko_ui::chrome::about_menu(
        &header,
        oiko_ui::chrome::ProductInfo {
            name: "Oiko Wow",
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

fn motion_display(
    ui: &mut egui::Ui,
    palette: Palette,
    display: &ModulationDisplay,
    display_scale: f32,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 106.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, palette.page);
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, palette.rule),
    );
    let (left, right) = display.snapshot();
    if left.len() < 32 {
        return;
    }
    let peak = left
        .iter()
        .chain(&right)
        .copied()
        .map(f32::abs)
        .fold(0.0_f32, f32::max)
        .max(1.0e-5);
    let plot = rect.shrink2(Vec2::new(0.0, 8.0));
    let stereo_is_visible = left
        .iter()
        .zip(&right)
        .any(|(left, right)| (left - right).abs() > peak * 1.0e-3);
    if stereo_is_visible {
        draw_history(ui, plot, &right, display_scale, palette.orange, 1.35);
    }
    draw_history(ui, plot, &left, display_scale, palette.blue, 1.6);
}

fn draw_history(ui: &egui::Ui, rect: Rect, values: &[f32], scale: f32, color: Color32, width: f32) {
    let last = (values.len() - 1) as f32;
    let points = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = rect.left() + index as f32 / last * rect.width();
            let y = rect.center().y - (*value / scale).clamp(-1.0, 1.0) * rect.height() * 0.43;
            Pos2::new(x, y)
        })
        .collect();
    ui.painter()
        .add(Shape::line(points, Stroke::new(width, color)));
}

fn knob<P: Param>(
    ui: &mut egui::Ui,
    label: &str,
    param: &P,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    palette: Palette,
) {
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(oiko_ui::typography::TEXT_SMALL)
                .color(palette.muted),
        );
        knob_face(ui, param, setter, palette);
        ui.label(
            egui::RichText::new(param.to_string())
                .size(oiko_ui::typography::TEXT_SMALL)
                .color(palette.ink),
        );
    });
}

fn knob_face<P: Param>(
    ui: &mut egui::Ui,
    param: &P,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    palette: Palette,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(58.0), Sense::hover());
    let response = ui.interact(
        rect,
        Id::new(("knob", param.as_ptr())),
        Sense::click_and_drag(),
    );
    parameter_drag(ui, &response, param, setter, knob_drag_mode());
    if response.double_clicked() {
        setter.set_discrete_parameter(param, param.default_plain_value());
    }
    draw_knob(
        ui,
        rect,
        &response,
        param.modulated_normalized_value(),
        palette,
    );
}

fn knob_drag_mode() -> DragMode {
    DragMode::Relative {
        sensitivity: 0.0045,
        horizontal_weight: 0.35,
        fine_scale: 0.2,
    }
}

fn draw_knob(
    ui: &mut egui::Ui,
    rect: Rect,
    response: &Response,
    normalized: f32,
    palette: Palette,
) {
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let center = rect.center();
    ui.painter().circle_filled(center, 28.0, palette.page);
    ui.painter()
        .circle_stroke(center, 28.0, Stroke::new(1.0, palette.rule));
    let angle = PI * 0.75 + normalized.clamp(0.0, 1.0) * PI * 1.5;
    ui.painter().line_segment(
        [
            center + Vec2::angled(angle) * 7.0,
            center + Vec2::angled(angle) * 22.0,
        ],
        Stroke::new(2.0, palette.blue),
    );
}

struct RateControl<'a> {
    free: &'a nice_plug::params::FloatParam,
    sync: &'a nice_plug::params::BoolParam,
    division: &'a nice_plug::params::EnumParam<crate::RateDivision>,
    range: crate::rate_sync::RateRange,
}

fn rate_knob(
    ui: &mut egui::Ui,
    label: &str,
    control: RateControl<'_>,
    tempo: f32,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    palette: Palette,
) {
    ui.push_id(label, |ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(oiko_ui::typography::TEXT_SMALL)
                    .color(palette.muted),
            );
            let value = if control.sync.value() {
                let effective = control.division.value().bounded(tempo, control.range);
                let position = control.free.preview_normalized(effective.rate_hz(tempo));
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(58.0), Sense::hover());
                let response = ui.interact(
                    rect,
                    Id::new(("knob", control.division.as_ptr())),
                    Sense::click_and_drag(),
                );
                parameter_drag_mapped(
                    ui,
                    &response,
                    control.division,
                    setter,
                    knob_drag_mode(),
                    (position, |normalized| {
                        crate::RateDivision::closest_to_hz(
                            control.free.preview_plain(normalized),
                            tempo,
                            control.range,
                        )
                    }),
                );
                if response.double_clicked() {
                    setter.set_discrete_parameter(
                        control.division,
                        crate::RateDivision::closest_to_hz(
                            control.free.default_plain_value(),
                            tempo,
                            control.range,
                        ),
                    );
                }
                let text = control.division.normalized_value_to_string(
                    control.division.preview_normalized(effective),
                    true,
                );
                draw_knob(ui, rect, &response, position, palette);
                response.on_hover_text(format!(
                    "{:.2} Hz at {tempo:.1} BPM",
                    effective.rate_hz(tempo)
                ));
                text
            } else {
                knob_face(ui, control.free, setter, palette);
                control
                    .free
                    .normalized_value_to_string(control.free.modulated_normalized_value(), false)
            };
            rate_readout(ui, &value, control, tempo, setter, palette);
        });
    });
}

fn rate_readout(
    ui: &mut egui::Ui,
    value: &str,
    control: RateControl<'_>,
    tempo: f32,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    palette: Palette,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(84.0, 14.0), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + 42.0, rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        FontId::proportional(oiko_ui::typography::TEXT_SMALL),
        palette.ink,
    );
    let synced = control.sync.value();
    for (sync, x, id) in [(false, 46.0, "rate-free"), (true, 66.0, "rate-sync")] {
        let choice = Rect::from_min_size(rect.min + Vec2::new(x, 0.0), Vec2::new(18.0, 14.0));
        let response = ui
            .interact(choice, Id::new((id, control.sync.as_ptr())), Sense::click())
            .on_hover_text(if sync {
                "Sync this rate to host tempo"
            } else {
                "Set this rate in Hz"
            });
        if response.hovered() {
            ui.painter().rect_filled(choice, 2.0, palette.field);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let color = if sync == synced {
            palette.blue
        } else if response.hovered() {
            palette.ink
        } else {
            palette.muted
        };
        if sync {
            // Match the eight-point cap height of the adjacent 11-point Hz text.
            let center = choice.center() + Vec2::new(0.0, 1.0);
            ui.painter().rect_filled(
                Rect::from_min_size(center + Vec2::new(1.0, -4.0), Vec2::new(1.0, 7.0)),
                0.0,
                color,
            );
            ui.painter().add(Shape::ellipse_filled(
                center + Vec2::new(-0.25, 2.5),
                Vec2::new(2.25, 1.5),
                color,
            ));
        } else {
            ui.painter().text(
                choice.center(),
                Align2::CENTER_CENTER,
                "Hz",
                FontId::proportional(oiko_ui::typography::TEXT_SMALL),
                color,
            );
        }
        if response.clicked() && sync != synced {
            if sync {
                setter.set_discrete_parameter(
                    control.division,
                    crate::RateDivision::closest_to_hz(control.free.value(), tempo, control.range),
                );
            } else {
                setter.set_discrete_parameter(
                    control.free,
                    control
                        .division
                        .value()
                        .bounded(tempo, control.range)
                        .rate_hz(tempo),
                );
            }
            setter.set_discrete_parameter(control.sync, sync);
        }
    }
    ui.painter().line_segment(
        [
            Pos2::new(rect.left() + 65.0, rect.top() + 3.0),
            Pos2::new(rect.left() + 65.0, rect.bottom() - 3.0),
        ],
        Stroke::new(1.0, palette.rule),
    );
}

fn parameter_slider<P: Param>(
    ui: &mut egui::Ui,
    label: &str,
    param: &P,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    palette: Palette,
    inverted: bool,
) {
    let (label_rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    ui.painter().text(
        Pos2::new(label_rect.left() + 8.0, label_rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(
            oiko_ui::typography::TEXT_SMALL,
            egui::FontFamily::Proportional,
        ),
        palette.muted,
    );
    ui.painter().text(
        Pos2::new(label_rect.right() - 8.0, label_rect.center().y),
        Align2::RIGHT_CENTER,
        param.to_string(),
        FontId::new(
            oiko_ui::typography::TEXT_SMALL,
            egui::FontFamily::Proportional,
        ),
        palette.ink,
    );
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 24.0),
        Sense::click_and_drag(),
    );
    let response = ui.interact(
        rect,
        Id::new(("slider", param.as_ptr())),
        Sense::click_and_drag(),
    );
    let value_range = rect.shrink2(Vec2::new(8.0, 0.0));
    parameter_absolute(ui, &response, value_range, param, setter, inverted);
    let track = Rect::from_center_size(value_range.center(), Vec2::new(value_range.width(), 4.0));
    ui.painter().rect_filled(track, 2.0, palette.track);
    let normalized = param.modulated_normalized_value().clamp(0.0, 1.0);
    let normalized = if inverted {
        1.0 - normalized
    } else {
        normalized
    };
    let x = value_range.left() + normalized * value_range.width();
    let filled = Rect::from_min_max(track.left_top(), Pos2::new(x, track.bottom()));
    ui.painter().rect_filled(filled, 2.0, palette.blue);
    let thumb = Pos2::new(x, rect.center().y);
    ui.painter().circle_filled(thumb, 7.5, palette.track);
    ui.painter().circle_stroke(
        thumb,
        7.5,
        Stroke::new(
            1.0,
            if response.hovered() || response.dragged() {
                palette.muted
            } else {
                palette.rule
            },
        ),
    );
    ui.painter().circle_filled(thumb, 2.75, palette.blue);
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

fn parameter_absolute<P: Param>(
    ui: &egui::Ui,
    response: &Response,
    value_range: Rect,
    param: &P,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    inverted: bool,
) {
    if response.double_clicked() {
        setter.set_discrete_parameter(param, param.default_plain_value());
        return;
    }
    let map = |value: f32| if inverted { 1.0 - value } else { value };
    parameter_drag_mapped(
        ui,
        response,
        param,
        setter,
        DragMode::Absolute {
            range: value_range,
            fine_sensitivity: None,
        },
        (map(param.unmodulated_normalized_value()), |v| {
            param.preview_plain(map(v))
        }),
    );
    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let normalized = ((pointer.x - value_range.left()) / value_range.width()).clamp(0.0, 1.0);
        setter.set_discrete_parameter(param, param.preview_plain(map(normalized)));
    }
}

fn footer(
    ui: &mut egui::Ui,
    palette: Palette,
    params: &WowParams,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
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
    let seed_rect = Rect::from_min_size(
        Pos2::new(
            rect.right() - 90.0 - oiko_ui::resize_grip::RESERVED_WIDTH,
            rect.center().y - height * 0.5,
        ),
        Vec2::new(90.0, height),
    );
    let range_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.center().y),
        Vec2::new(176.0, height),
    );
    let quality_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 16.0, rect.center().y - height * 0.5),
        Vec2::new(120.0, height),
    );
    footer_param(
        ui,
        quality_rect,
        "QUALITY",
        72.0,
        &params.quality,
        setter,
        palette,
    );
    footer_param(
        ui,
        range_rect,
        "PITCH RANGE",
        104.0,
        &params.depth_behavior,
        setter,
        palette,
    );
    footer_param(
        ui,
        seed_rect,
        "SEED",
        58.0,
        &params.random_seed,
        setter,
        palette,
    );
}

fn footer_param<P: Param>(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    value_width: f32,
    param: &P,
    setter: &oiko_plugin::gestures::GestureSetter<'_>,
    palette: Palette,
) {
    let value_rect = Rect::from_min_max(
        Pos2::new(rect.right() - value_width, rect.top() + 1.0),
        Pos2::new(rect.right(), rect.bottom() - 1.0),
    );
    let left_button = Rect::from_min_max(
        value_rect.left_top(),
        Pos2::new(value_rect.left() + 20.0, value_rect.bottom()),
    );
    let right_button = Rect::from_min_max(
        Pos2::new(value_rect.right() - 20.0, value_rect.top()),
        value_rect.right_bottom(),
    );
    let whole = ui.interact(value_rect, Id::new(("footer-param", label)), Sense::click());
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
    let hovered = whole.hovered() || previous.hovered() || next.hovered();
    ui.painter().rect_filled(
        value_rect,
        3.0,
        if hovered { palette.track } else { palette.page },
    );
    ui.painter().text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(
            oiko_ui::typography::TEXT_SMALL,
            egui::FontFamily::Proportional,
        ),
        palette.muted,
    );
    ui.painter().text(
        Pos2::new(value_rect.left() + 7.0, value_rect.center().y),
        Align2::LEFT_CENTER,
        "‹",
        FontId::new(
            oiko_ui::typography::TEXT_SMALL,
            egui::FontFamily::Proportional,
        ),
        if !can_go_previous {
            palette.rule
        } else if previous.hovered() {
            palette.ink
        } else {
            palette.muted
        },
    );
    ui.painter().text(
        Pos2::new(value_rect.right() - 7.0, value_rect.center().y),
        Align2::RIGHT_CENTER,
        "›",
        FontId::new(
            oiko_ui::typography::TEXT_SMALL,
            egui::FontFamily::Proportional,
        ),
        if !can_go_next {
            palette.rule
        } else if next.hovered() {
            palette.ink
        } else {
            palette.muted
        },
    );
    ui.painter().text(
        value_rect.center(),
        Align2::CENTER_CENTER,
        param.to_string(),
        FontId::new(
            oiko_ui::typography::TEXT_SMALL,
            egui::FontFamily::Proportional,
        ),
        palette.ink,
    );
    if (previous.hovered() && can_go_previous)
        || (next.hovered() && can_go_next)
        || (whole.hovered() && can_go_next)
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if can_go_next && (next.clicked() || (whole.clicked() && !previous.clicked())) {
        setter.set_discrete_parameter(
            param,
            param.next_step(param.unmodulated_plain_value(), false),
        );
    } else if can_go_previous && (previous.clicked() || whole.secondary_clicked()) {
        setter.set_discrete_parameter(
            param,
            param.previous_step(param.unmodulated_plain_value(), false),
        );
    }
}

#[cfg(test)]
mod tests;
