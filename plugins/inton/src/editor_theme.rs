//! Oiko palette and typography, shared visually with Weft.
use egui::FontId;
pub use oiko_ui::typography::{TEXT_BODY, TEXT_HEADING, TEXT_SMALL, TEXT_TITLE};

pub fn compact_rows(ui: &mut egui::Ui) {
    ui.style_mut().override_font_id = Some(FontId::proportional(TEXT_SMALL));
    ui.spacing_mut().item_spacing.y = 1.;
    ui.spacing_mut().button_padding.y = 1.;
    ui.spacing_mut().interact_size.y = 18.;
}
pub use oiko_ui::theme::{Palette, apply_theme};
