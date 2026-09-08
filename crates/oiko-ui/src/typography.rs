//! Four typography roles, one font family, shared by every Oiko editor.
use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::sync::Arc;

pub const TEXT_SMALL: f32 = 11.0;
pub const TEXT_BODY: f32 = 14.0;
pub const TEXT_HEADING: f32 = 18.0;
pub const TEXT_TITLE: f32 = 20.0;

/// Install on editor creation/theme change, not per frame. Both themes use Ubuntu.
pub fn install_fonts(context: &egui::Context, _dark: bool) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Ubuntu-Regular".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Ubuntu-Regular.ttf"
        ))),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "Ubuntu-Regular".into());
    }
    context.set_fonts(fonts);
}

pub fn apply_text_styles(style: &mut egui::Style) {
    for (role, size) in [
        (TextStyle::Body, TEXT_BODY),
        (TextStyle::Button, TEXT_SMALL),
        (TextStyle::Small, TEXT_SMALL),
        (TextStyle::Heading, TEXT_HEADING),
        (TextStyle::Monospace, TEXT_SMALL),
    ] {
        style.text_styles.insert(role, FontId::proportional(size));
    }
}
