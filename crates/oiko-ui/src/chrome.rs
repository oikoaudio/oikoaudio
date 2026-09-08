//! Shared header and About/zoom menu. Product-specific help follows common controls.
use crate::{layout, scale, theme::Palette, typography::*};
use egui::{Align2, FontId, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};

pub struct HeaderResponse {
    pub anchor: Response,
    pub title_rect: Rect,
    pub toggle_about: bool,
    pub theme_changed: bool,
}

pub fn header(ui: &mut Ui, title: &str, dark: &mut bool) -> HeaderResponse {
    let p = Palette::new(*dark);
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), layout::HEADER_HEIGHT),
        Sense::hover(),
    );
    let title_rect = Rect::from_min_max(rect.left_top(), pos2(rect.left() + 100.0, rect.bottom()));
    let title_response = ui
        .interact(
            title_rect,
            ui.id().with("oiko-header-title"),
            Sense::click(),
        )
        .on_hover_text("About and interface scale");
    ui.painter().text(
        pos2(rect.left() + layout::INSET, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(TEXT_HEADING),
        p.ink,
    );
    let brand_rect = Rect::from_center_size(rect.center(), vec2(100.0, 32.0));
    let anchor = ui
        .interact(
            brand_rect,
            ui.id().with("oiko-header-about"),
            Sense::click(),
        )
        .on_hover_text("About and interface scale");
    ui.painter().text(
        brand_rect.center(),
        Align2::CENTER_CENTER,
        "OIKO AUDIO",
        FontId::proportional(TEXT_SMALL),
        if anchor.hovered() { p.ink } else { p.muted },
    );
    let secondary = ui.rect_contains_pointer(rect)
        && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
    let toggle_about = title_response.clicked() || anchor.clicked() || secondary;
    let theme_rect = Rect::from_center_size(
        pos2(rect.right() - layout::INSET - 12.0, rect.center().y),
        vec2(30.0, 28.0),
    );
    let theme = ui
        .interact(
            theme_rect,
            ui.id().with("oiko-header-theme"),
            Sense::click(),
        )
        .on_hover_text(if *dark {
            "Switch to light theme"
        } else {
            "Switch to dark theme"
        });
    if title_response.hovered() || anchor.hovered() || theme.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if theme.hovered() {
        ui.painter().rect_filled(theme_rect, 3.0, p.track);
    }
    let center = theme_rect.center();
    if *dark {
        ui.painter().circle_filled(center, 6.0, p.muted);
        ui.painter().circle_filled(
            center + vec2(3.0, -2.0),
            5.5,
            if theme.hovered() { p.track } else { p.panel },
        );
    } else {
        ui.painter()
            .circle_stroke(center, 4.5, Stroke::new(1.4, p.muted));
        for i in 0..8 {
            let d = Vec2::angled(i as f32 * std::f32::consts::TAU / 8.0);
            ui.painter().line_segment(
                [center + d * 7.0, center + d * 9.0],
                Stroke::new(1.2, p.muted),
            );
        }
    }
    if theme.clicked() {
        *dark = !*dark;
    }
    HeaderResponse {
        anchor,
        title_rect,
        toggle_about,
        theme_changed: theme.clicked(),
    }
}

pub struct ProductInfo<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub website: &'a str,
}
pub struct AboutResponse {
    pub is_open: bool,
    pub scale: Option<f32>,
}

pub fn about_menu(
    header: &HeaderResponse,
    product: ProductInfo<'_>,
    current_scale: f32,
    extra: impl FnOnce(&mut Ui),
) -> AboutResponse {
    // Popups use viewport coordinates even when the editor canvas has a layer transform.
    let viewport = header.anchor.ctx.content_rect();
    let frame = egui::Frame::popup(&header.anchor.ctx.global_style());
    let margin = frame.total_margin().sum();
    let width = 304.0_f32.min(viewport.width() - layout::GAP * 2.0);
    let content_width = (width - margin.x).max(1.0);
    let content_height = (viewport.height() - layout::GAP * 2.0 - margin.y).max(1.0);
    let popup = egui::Popup::menu(&header.anchor)
        .width(width)
        .frame(frame)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .open_memory(header.toggle_about.then_some(egui::SetOpenCommand::Toggle));
    let id = popup.get_id();
    let mut requested = None;
    popup.show(|ui| {
        ui.set_width(content_width);
        let top = ui.cursor().top();
        ui.label(egui::RichText::new(product.name).size(TEXT_HEADING));
        ui.label("Interface scale");
        let button_width = ui
            .painter()
            .layout_no_wrap(
                "200%".into(),
                egui::TextStyle::Button.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x
            + ui.spacing().button_padding.x * 2.0;
        let gap = 3.0;
        let count = scale::UI_SCALE_STEPS.len();
        let per_row =
            (((content_width + gap) / (button_width + gap)).floor() as usize).clamp(1, count);
        for choices in scale::UI_SCALE_STEPS.chunks(per_row) {
            let (row, _) =
                ui.allocate_exact_size(vec2(content_width, layout::CONTROL_HEIGHT), Sense::hover());
            let columns = layout::Columns::equal(row.left(), row.width(), per_row, gap);
            for (index, &value) in choices.iter().enumerate() {
                let response = ui.put(
                    columns.cell(index, row.top(), row.height()),
                    egui::Button::new(format!("{}%", (value * 100.0) as u32))
                        .selected(scale::closest_ui_scale(current_scale) == value),
                );
                if response.clicked() {
                    requested = Some(value);
                    ui.close();
                }
            }
        }
        ui.separator();
        // Keep zoom reachable while longer About/help text scrolls in a small window.
        let remaining = (content_height - (ui.cursor().top() - top)).max(0.0);
        egui::ScrollArea::vertical()
            .max_height(remaining)
            .min_scrolled_height(0.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.label(format!("Version {}", product.version));
                ui.label("Oiko Audio");
                ui.hyperlink_to("Website", product.website);
                extra(ui);
            });
    });
    AboutResponse {
        is_open: egui::Popup::is_id_open(&header.anchor.ctx, id),
        scale: requested,
    }
}
