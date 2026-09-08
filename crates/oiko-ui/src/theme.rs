//! Fixed shared Oiko palette and widget styling.
use egui::{Color32, CornerRadius, Stroke, Vec2};
#[derive(Clone, Copy)]
pub struct Palette {
    pub page: Color32,
    pub panel: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub rule: Color32,
    pub field: Color32,
    pub track: Color32,
    pub orange: Color32,
    pub warning: Color32,
    pub blue: Color32,
    pub key: Color32,
    pub accidental: Color32,
    pub octave_key: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                page: Color32::from_rgb(23, 24, 23),
                panel: Color32::from_rgb(32, 34, 32),
                ink: Color32::from_rgb(238, 238, 232),
                muted: Color32::from_rgb(167, 170, 161),
                rule: Color32::from_rgb(58, 61, 56),
                field: Color32::from_rgb(23, 24, 23),
                track: Color32::from_rgb(48, 51, 47),
                orange: Color32::from_rgb(255, 173, 92),
                warning: Color32::from_rgb(235, 91, 70),
                blue: Color32::from_rgb(91, 170, 207),
                key: Color32::from_rgb(42, 45, 41),
                accidental: Color32::from_rgb(26, 28, 26),
                octave_key: Color32::from_rgb(52, 55, 50),
            }
        } else {
            Self {
                page: Color32::from_rgb(232, 232, 229),
                panel: Color32::from_rgb(246, 246, 242),
                ink: Color32::from_rgb(22, 23, 21),
                muted: Color32::from_rgb(74, 77, 70),
                rule: Color32::from_rgb(174, 179, 170),
                field: Color32::from_rgb(215, 217, 211),
                track: Color32::from_rgb(197, 201, 192),
                orange: Color32::from_rgb(166, 61, 5),
                warning: Color32::from_rgb(176, 42, 32),
                blue: Color32::from_rgb(18, 55, 151),
                key: Color32::from_rgb(225, 227, 221),
                accidental: Color32::from_rgb(165, 170, 160),
                octave_key: Color32::from_rgb(197, 201, 192),
            }
        }
    }
}

pub fn apply_theme(context: &egui::Context, dark: bool) {
    crate::typography::install_fonts(context, dark);

    let palette = Palette::new(dark);
    let theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    context.set_theme(theme);
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.field;
    visuals.faint_bg_color = palette.track;
    visuals.window_stroke = Stroke::new(1.0, palette.rule);
    visuals.override_text_color = Some(palette.ink);
    visuals.selection.bg_fill = palette.blue;
    visuals.selection.stroke = Stroke::new(1.0, palette.ink);
    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.rule);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.ink);
    visuals.widgets.inactive.bg_fill = palette.field;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.rule);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.muted);
    visuals.widgets.hovered.bg_fill = palette.track;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.ink);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.ink);
    visuals.widgets.active.bg_fill = palette.track;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.ink);
    visuals.window_corner_radius = CornerRadius::same(6);
    context.set_visuals_of(theme, visuals);

    let mut style = (*context.style_of(theme)).clone();
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.spacing.scroll.foreground_color = true;
    style.spacing.scroll.dormant_handle_opacity = 0.65;
    style.spacing.scroll.active_handle_opacity = 0.85;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(7.0, 4.0);
    crate::typography::apply_text_styles(&mut style);
    context.set_style_of(theme, style);
}
