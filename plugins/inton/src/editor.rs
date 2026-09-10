mod rendering;
use crate::editor_model::{
    AssignmentIntent, BrowserTab, Inspection, Overlay, ScaleShape, SetView, ViewPreferences,
};
use crate::editor_theme::{Palette, TEXT_BODY, TEXT_HEADING, TEXT_SMALL, TEXT_TITLE, apply_theme};
use crate::library::{Library, Preferences, preferences_path};
use crate::plugin::HostRef;
use egui::{Align2, FontId, RichText, ScrollArea, Sense, Stroke, pos2, vec2};
use inton_core::engine::Parameter;
use inton_core::mts::MasterStatus;
use inton_core::runtime::Shared;
use inton_core::state::Project;
use inton_core::tuning::ValidatedScale;
use keyboard_types::Key as HostKey;
use nice_plug::context::gui::GuiContext;
use nice_plug_egui::baseview::HandlerError;
use nice_plug_egui::{Frame, KeyCapture, NiceEguiApp};
use rendering::{Icon, draw_scale_compared, draw_scale_sized, icon, small_icon};
use scale_editor::ScaleEdit;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
mod browser;
mod scale_editor;
mod scale_set;

#[derive(Clone)]
struct ScaleDrag(String);
#[derive(Clone)]
struct SlotDrag(usize);
#[derive(Clone)]
enum Command {
    Preview(String),
    Assign(String, usize),
    Append(String),
    Rescan,
    Import,
}
type JobResult = (
    Arc<Library>,
    Command,
    Result<Option<(String, ValidatedScale)>, String>,
);
pub struct Editor {
    edit: Option<ScaleEdit>,
    shared: Arc<Shared>,
    host: HostRef,
    library: Option<Arc<Library>>,
    preferences: Preferences,
    folder_text: String,
    job: Option<JoinHandle<JobResult>>,
    queue: VecDeque<Command>,
    search: String,
    section: usize,
    category: String,
    inspection: Inspection,
    browser_cursor: Option<String>,
    library_scroll: f32,
    library_reveal: Option<String>,
    preview: Option<ValidatedScale>,
    preview_error: Option<String>,
    set: SetView,
    browser_tab: BrowserTab,
    assignment: AssignmentIntent,
    focus_library: bool,
    message: Option<String>,
    overlay: Overlay,
    reference_ring: bool,
    styled: Option<bool>,
    acknowledgment: Option<(usize, Instant)>,
    last_active: Option<usize>,
    last_destination: Option<usize>,
}
impl Editor {
    pub fn new(shared: Arc<Shared>, host: HostRef) -> Self {
        let preferences = Preferences::load_from(&preferences_path()).unwrap_or_default();
        let initial_slot = shared.parameters.read().unwrap_or_default().position;
        let mut e = Self {
            edit: None,
            shared,
            host,
            folder_text: preferences.folder.display().to_string(),
            preferences,
            library: Some(Arc::new(Library::factory())),
            job: None,
            queue: VecDeque::new(),
            search: String::new(),
            section: 0,
            category: "All categories".into(),
            inspection: Inspection::Active,
            browser_cursor: None,
            library_scroll: 0.,
            library_reveal: None,
            preview: None,
            preview_error: None,
            set: SetView {
                destination: initial_slot,
                ..SetView::default()
            },
            browser_tab: BrowserTab::Library,
            assignment: AssignmentIntent::Browse,
            focus_library: false,
            message: None,
            overlay: Overlay::None,
            reference_ring: false,
            styled: None,
            acknowledgment: None,
            last_active: None,
            last_destination: None,
        };
        e.enqueue(Command::Rescan);
        e
    }
    fn enqueue(&mut self, command: Command) {
        // New browsing intent supersedes queued previews, never explicit assignments.
        if matches!(command, Command::Preview(_)) {
            self.queue.retain(|c| !matches!(c, Command::Preview(_)));
        }
        self.queue.push_back(command);
        self.start_next();
    }
    fn start_next(&mut self) {
        if self.job.is_some() {
            return;
        }
        let Some(command) = self.queue.pop_front() else {
            return;
        };
        let Some(mut lib) = self.library.as_ref().cloned() else {
            return;
        };
        let folder = self.preferences.folder.clone();
        self.job = Some(thread::spawn(move || {
            let result = (|| match &command {
                Command::Rescan => {
                    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                    Arc::make_mut(&mut lib).rescan(&folder)?;
                    Ok(None)
                }
                Command::Import => {
                    let Some(source) = file_dialog(false)? else {
                        return Ok(None);
                    };
                    let id = Arc::make_mut(&mut lib).import(&source, &folder)?;
                    let scale = lib.load_prepared(&id)?;
                    Ok(Some((id, scale)))
                }
                Command::Preview(id) | Command::Assign(id, _) | Command::Append(id) => {
                    let scale = lib.load_prepared(id)?;
                    Ok(Some((id.clone(), scale)))
                }
            })();
            (lib, command, result)
        }));
    }
    pub fn select(&mut self, id: String) {
        self.preview_error = None;
        self.inspection = Inspection::Library(id.clone());
        self.browser_cursor = Some(id.clone());
        self.preview = None;
        self.enqueue(Command::Preview(id));
    }
    fn follow_active_scale(&mut self) {
        self.shared.stop_audition();
        self.inspection = Inspection::Active;
        self.preview = None;
        self.preview_error = None;
        self.assignment = AssignmentIntent::Browse;
        self.set.destination = self.shared.snapshot().parameters.position;
    }
    pub fn show_project(&mut self) {
        self.follow_active_scale();
        self.browser_tab = BrowserTab::Set;
        self.set_view(ViewPreferences {
            browser_open: true,
            ..self.preferences.view
        });
    }
    fn begin_add_scale(&mut self) {
        self.browser_tab = BrowserTab::Library;
        self.shared.enable_scale_set();
        self.host.request_callback();
        self.assignment = AssignmentIntent::Append;
        self.focus_library = true;
        if !self.preferences.view.browser_open {
            self.set_view(ViewPreferences {
                browser_open: true,
                ..self.preferences.view
            });
        }
    }
    fn begin_replace_scale(&mut self, slot: usize) {
        self.inspect_slot(slot);
        self.begin_add_scale();
        self.assignment = if self.shared.snapshot().slots[slot].is_some() {
            AssignmentIntent::Replace(slot)
        } else {
            AssignmentIntent::Browse
        };
    }
    fn library_will_append(&self) -> bool {
        if !self.shared.snapshot().uses_scale_set() {
            return false;
        }
        match self.assignment {
            AssignmentIntent::Append => true,
            AssignmentIntent::Replace(_) => false,
            AssignmentIntent::Browse => {
                self.shared.snapshot().slots[self.set.destination].is_some()
            }
        }
    }
    fn assign_from_library(&mut self, id: String, slot: usize) {
        if self.direct_library_load() {
            self.assign_scale(id, slot);
            return;
        }
        if self.library_will_append() {
            self.enqueue(Command::Append(id));
        } else {
            let slot = match self.assignment {
                AssignmentIntent::Replace(target) => target,
                _ if self.shared.snapshot().uses_scale_set() => slot,
                _ => 0,
            };
            self.assignment = AssignmentIntent::Browse;
            self.assign_scale(id, slot);
        }
    }
    fn assign_scale(&mut self, id: String, slot: usize) {
        self.enqueue(Command::Assign(id, slot));
    }
    fn poll(&mut self) {
        if self.job.as_ref().is_some_and(|j| j.is_finished()) {
            match self.job.take().unwrap().join() {
                Ok((lib, command, result)) => {
                    self.library = Some(lib);
                    match result {
                        Ok(Some((id, scale))) => {
                            match command {
                                Command::Assign(_, _) | Command::Append(_) => {
                                    let assigned = match command {
                                        Command::Assign(_, slot) => self
                                            .shared
                                            .assign_prepared(slot, scale.clone())
                                            .map(|()| slot),
                                        _ => self.shared.append_prepared(scale.clone()),
                                    };
                                    match assigned {
                                        Ok(slot) => {
                                            self.assignment = AssignmentIntent::Browse;
                                            self.set.destination = slot;
                                            self.inspection = Inspection::Slot(slot);
                                            self.preview = None;
                                            self.shared.stop_audition();
                                            self.message = None;
                                            self.acknowledgment = Some((slot, Instant::now()));
                                            self.host.request_callback();
                                        }
                                        Err(e) => self.message = Some(e),
                                    }
                                }
                                Command::Import => {
                                    self.inspection = Inspection::Active;
                                    self.inspection = Inspection::Library(id.clone());
                                    self.preview = Some(scale.clone());
                                }
                                _ => {}
                            }
                            if self.inspection.library() == Some(&id) {
                                self.preview_error = None;
                                self.message = None;
                                self.shared.audition_prepared(scale.clone());
                                self.preview = Some(scale);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            if matches!(&command, Command::Preview(id) | Command::Assign(id, _) | Command::Append(id) if self.inspection.library() == Some(id))
                            {
                                self.preview_error = Some(e.clone());
                            }
                            self.message = Some(e);
                        }
                    }
                }
                Err(_) => {
                    self.library = Some(Arc::new(Library::factory()));
                    self.message = Some("Library task failed. Rescan to retry.".into());
                    if self.inspection.library().is_some() && self.preview.is_none() {
                        self.preview_error = self.message.clone();
                    }
                }
            }
            self.start_next();
        }
    }
    fn set_parameter(&self, id: Parameter, value: f64) {
        self.shared.edit_parameter(id, value);
        self.host.request_callback();
    }
    fn save_preferences(&mut self) {
        if let Err(e) = self.preferences.save_to(&preferences_path()) {
            self.message = Some(e);
        }
    }
    fn set_view(&mut self, view: ViewPreferences) {
        self.preferences.view = view;
        self.save_preferences();
    }
    fn header(&mut self, ui: &mut egui::Ui, p: Palette) {
        let mut dark = self.preferences.view.dark;
        let header = oiko_ui::chrome::header(ui, "inton", &mut dark);
        #[cfg(test)]
        {
            trace(ui, "brand-menu", header.anchor.rect);
            trace(ui, "header-title", header.title_rect);
        }
        if header.theme_changed {
            self.preferences.view.dark = dark;
            self.save_preferences();
        }
        let response = oiko_ui::chrome::about_menu(
            &header,
            oiko_ui::chrome::ProductInfo {
                name: "Oiko Inton",
                version: env!("CARGO_PKG_VERSION"),
                website: "https://oikoaudio.com/inton/",
            },
            self.preferences.view.scale as f32,
            |ui| {
                ui.separator();
                let guide = ui.button("Listening guide");
                #[cfg(test)]
                trace(ui, "listening-guide", guide.rect);
                if guide.clicked() {
                    self.overlay = Overlay::ListeningGuide;
                    ui.close();
                }
                if ui.button("Quick guide").clicked() {
                    self.overlay = Overlay::QuickGuide;
                    ui.close();
                }
                let status = self.shared.status.lock().unwrap().0;
                if status == MasterStatus::Unavailable {
                    ui.label("MTS-ESP needs its shared library. See the installation guide, then restart the host.");
                }
                if let Some(message) = &self.message {
                    ui.label(RichText::new(message).color(p.orange));
                }
            },
        );
        if let Some(scale) = response.scale {
            let scale_changed =
                ViewPreferences::nearest_scale(self.preferences.view.scale) != f64::from(scale);
            self.set_view(ViewPreferences {
                scale: scale as f64,
                ..self.preferences.view
            });
            if scale_changed && self.host.is_connected() {
                oiko_ui::scale::request_scale(
                    ui.ctx(),
                    scale,
                    vec2(
                        self.preferences.view.width() as f32,
                        self.preferences.view.height() as f32,
                    ),
                );
            }
        }
    }
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if self.edit.is_some() && !ui.ctx().text_edit_focused() {
            let redo = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
            if ui.input_mut(|input| input.consume_shortcut(&redo)) {
                self.draft_history_action(true);
            } else if ui.input_mut(|input| input.consume_shortcut(&undo)) {
                self.draft_history_action(false);
            }
        }
        if self.preferences.view.browser_open
            && self.overlay == Overlay::None
            && self.edit.is_none()
            && !matches!(self.assignment, AssignmentIntent::Replace(_))
            && !egui::Popup::is_any_open(ui.ctx())
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.set_view(ViewPreferences {
                browser_open: false,
                ..self.preferences.view
            });
        }
        if self.overlay == Overlay::None
            && self.edit.is_none()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::B))
        {
            self.focus_library =
                !self.preferences.view.browser_open && self.browser_tab != BrowserTab::Set;
            self.set_view(ViewPreferences {
                browser_open: !self.preferences.view.browser_open,
                ..self.preferences.view
            });
        }
        if self.job.is_none()
            && self.queue.is_empty()
            && self.edit.is_none()
            && self.overlay == Overlay::None
            && !ui.ctx().text_edit_focused()
            && !egui::Popup::is_any_open(ui.ctx())
        {
            if ui.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Y,
                ))
            }) {
                self.set_history_action(1);
            } else if ui.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Z,
                ))
            }) {
                self.set_history_action(-1);
            }
        }
        if self.styled != Some(self.preferences.view.dark) {
            apply_theme(ui.ctx(), self.preferences.view.dark);
            self.styled = Some(self.preferences.view.dark);
        }
        ui.set_style(ui.ctx().style_of(if self.preferences.view.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        }));
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        let p = Palette::new(self.preferences.view.dark);
        let parameters = self.shared.parameters.read().unwrap_or_default();
        let (project, name, empty, progress) = {
            let s = self.shared.project.lock().unwrap();
            (
                s.project.clone(),
                s.engine.name.clone(),
                s.engine.empty,
                s.engine.progress(),
            )
        };
        let (status, clients) = self
            .shared
            .status
            .try_lock()
            .map(|s| *s)
            .unwrap_or((MasterStatus::Unavailable, 0));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.page).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 0,
                bottom: 6,
            }))
            .show(ui, |ui| {
                let footer_bounds = ui.max_rect();
                self.header(ui, p);
                let center_area = egui::Rect::from_min_max(
                    pos2(ui.max_rect().left() - 12., ui.cursor().top()),
                    pos2(ui.max_rect().right() + 12., ui.max_rect().bottom() - 24.),
                );
                ui.painter().rect_filled(center_area, 0, p.panel);
                let body_height = (ui.available_height() - 30.).max(230.);
                ui.horizontal_top(|ui| {
                    let width = ui.available_width();
                    let action_window = ui.allocate_ui_with_layout(
                        vec2(width, body_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(width);
                            ui.set_min_height(body_height);
                            if self.edit.is_some() {
                                self.edit_panel(ui, p, body_height);
                                return;
                            }
                            self.scale_panel(ui, p, &project, parameters.position, &name, empty);
                            if progress < 1. {
                                ui.add(
                                    egui::ProgressBar::new(progress as f32)
                                        .desired_height(3.)
                                        .fill(p.orange),
                                );
                            }
                        },
                    );
                    #[cfg(test)]
                    trace(ui, "action-window", action_window.response.rect);
                    #[cfg(not(test))]
                    let _ = action_window;
                });
                self.footer(ui, p, parameters, status, clients, footer_bounds);
            });
        if self.overlay == Overlay::Settings {
            self.folder_settings(ui.ctx());
        }
        if self.overlay.is_guide() {
            self.show_listening_guide(ui.ctx());
        }
        if self.overlay == Overlay::Connection {
            self.show_connection_details(ui.ctx());
        }
    }
    fn footer(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        parameters: inton_core::engine::Parameters,
        status: MasterStatus,
        clients: usize,
        bounds: egui::Rect,
    ) {
        let rect = egui::Rect::from_min_max(
            pos2(bounds.left(), bounds.bottom() - 24.),
            pos2(bounds.right(), bounds.bottom() + 6.),
        );
        let center = rect.center().y;
        let controls_left = rect.left() + 3.;
        for (id, offset, width, label, label_width) in [
            (Parameter::Reference, 0., 96., "", 16.),
            (Parameter::Transpose, 104., 132., "Transpose", 68.),
        ] {
            let group = egui::Rect::from_center_size(
                pos2(controls_left + offset + width / 2., center),
                vec2(width, 22.),
            );
            if id == Parameter::Reference {
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("footer-fork")
                        .max_rect(egui::Rect::from_center_size(
                            pos2(group.left() + 8., center),
                            vec2(23., 23.),
                        ))
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                icon(
                    &mut child,
                    "reference-icon",
                    Icon::Fork,
                    "Reference frequency",
                    p,
                );
            } else {
                let label_rect = ui.painter().text(
                    group.left_center(),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(TEXT_SMALL),
                    p.muted,
                );
                #[cfg(test)]
                trace(ui, "footer-transpose-label", label_rect);
                #[cfg(not(test))]
                let _ = label_rect;
            }
            let label_width = if label.is_empty() {
                label_width
            } else {
                ui.painter()
                    .layout_no_wrap(label.into(), FontId::proportional(TEXT_SMALL), p.muted)
                    .size()
                    .x
                    + 6.
            };
            let field = egui::Rect::from_min_max(
                pos2(group.left() + label_width, group.top()),
                group.right_bottom(),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("footer-value", id.index()))
                    .max_rect(field)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            child.set_clip_rect(field);
            child.style_mut().override_font_id = Some(FontId::proportional(TEXT_SMALL));
            child.style_mut().drag_value_text_style = egui::TextStyle::Body;
            child.spacing_mut().interact_size.x = 0.;
            child.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
            child.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
            child.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
            child.visuals_mut().override_text_color = Some(p.ink);
            child.spacing_mut().button_padding = vec2(2., 2.);
            let mut v = parameters.value(id);
            let (range, speed, decimals, suffix, tip) = match id {
                Parameter::MorphTime => (
                    0.0..=10000.0,
                    10.,
                    0,
                    " ms",
                    "Duration of the next scale transition",
                ),
                Parameter::Reference => (400.0..=480.0, 0.05, 2, " Hz", "Reference frequency"),
                Parameter::Transpose => (-24.0..=24.0, 1., 0, " st", "Transpose in semitones"),
                Parameter::Position | Parameter::Enabled => {
                    unreachable!("Not a numeric footer control")
                }
            };
            let response = child
                .add(
                    egui::DragValue::new(&mut v)
                        .range(range)
                        .speed(speed)
                        .fixed_decimals(decimals)
                        .suffix(suffix),
                )
                .on_hover_text(tip);
            #[cfg(test)]
            trace(ui, &format!("footer-value:{}", id.index()), response.rect);
            if response.drag_started() {
                self.host.begin_parameter(id);
            }
            if response.changed() {
                self.set_parameter(id, v);
            }
        }
        let connection = egui::Rect::from_center_size(
            pos2(
                rect.right() - 62. - oiko_ui::resize_grip::RESERVED_WIDTH,
                center,
            ),
            vec2(124., 23.),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("footer-connection")
                .max_rect(egui::Rect::from_min_max(
                    pos2(connection.right() - 23., connection.top()),
                    connection.right_bottom(),
                ))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        if icon(
            &mut child,
            "power",
            Icon::Power(parameters.enabled),
            "Turn tuning output on or off",
            p,
        )
        .clicked()
        {
            self.set_parameter(Parameter::Enabled, if parameters.enabled { 0. } else { 1. });
        }
        let text = match status {
            MasterStatus::ActiveMaster => format!(
                "{clients} {}",
                if clients == 1 { "client" } else { "clients" }
            ),
            MasterStatus::Disabled => "Disabled".into(),
            MasterStatus::BlockedByOtherMaster => "Tuning blocked".into(),
            MasterStatus::Unavailable => "Unavailable".into(),
            MasterStatus::Error => "MTS error".into(),
        };
        let text_rect = egui::Rect::from_min_max(
            connection.left_top(),
            pos2(connection.right() - 29., connection.bottom()),
        );
        let blocked = status == MasterStatus::BlockedByOtherMaster;
        let response = ui.interact(text_rect,ui.id().with("connection-status"),if blocked {Sense::click()} else {Sense::hover()}).on_hover_text(if blocked {
            "Inton is not sending tuning. MTS reports another master, or a connection left behind after a crash. Click for recovery options.".to_string()
        } else {format!("{} · {clients} registered MTS clients.",status.label())});
        #[cfg(test)]
        trace(ui, "connection-status", response.rect);
        let paint = ui.painter().with_clip_rect(text_rect);
        if blocked {
            paint.rect_filled(
                text_rect,
                3,
                p.orange
                    .gamma_multiply(if response.hovered() { 0.16 } else { 0.08 }),
            );
            paint.rect_stroke(
                text_rect,
                3,
                Stroke::new(1., p.orange),
                egui::StrokeKind::Inside,
            );
            let c = pos2(text_rect.left() + 10., center);
            paint.add(egui::Shape::closed_line(
                vec![c + vec2(0., -6.), c + vec2(6., 5.), c + vec2(-6., 5.)],
                Stroke::new(1., p.orange),
            ));
            paint.line_segment(
                [c + vec2(0., -2.), c + vec2(0., 1.)],
                Stroke::new(1., p.orange),
            );
            paint.circle_filled(c + vec2(0., 3.), 0.7, p.orange);
        }
        if status == MasterStatus::ActiveMaster {
            let text_width = paint
                .layout_no_wrap(text.clone(), FontId::proportional(TEXT_SMALL), p.blue)
                .size()
                .x;
            let c = pos2(text_rect.right() - text_width - 12., center);
            let stroke = Stroke::new(1.2, p.blue);
            for sign in [-1., 1.] {
                paint.add(egui::Shape::line(
                    [
                        vec2(-2., -3.),
                        vec2(0., -5.),
                        vec2(3., -5.),
                        vec2(5., -3.),
                        vec2(5., 0.),
                        vec2(3., 2.),
                    ]
                    .map(|v| c + v * sign)
                    .to_vec(),
                    stroke,
                ));
            }
            paint.line_segment([c + vec2(-2., 2.), c + vec2(2., -2.)], stroke);
            #[cfg(test)]
            trace(
                ui,
                "connection-symbol",
                egui::Rect::from_center_size(c, vec2(12., 12.)),
            );
        }
        paint.text(
            text_rect.right_center() - vec2(if blocked { 5. } else { 0. }, 0.),
            Align2::RIGHT_CENTER,
            text,
            FontId::proportional(TEXT_SMALL),
            if blocked {
                p.orange
            } else if status == MasterStatus::ActiveMaster {
                p.blue
            } else {
                p.muted
            },
        );
        if response.clicked() {
            self.overlay = Overlay::Connection;
        }
    }

    fn set_history_action(&mut self, action: i8) {
        let result = if action == 0 {
            self.shared.clear_scale_set()
        } else {
            self.shared.undo_edit(action > 0)
        };
        match result {
            Ok(()) => {
                let active = self.shared.snapshot().parameters.position;
                self.inspect_slot(active);
                self.inspection = Inspection::Active;
                self.set.revealed.clear();
                self.last_active = None;
                self.last_destination = None;
                self.message = None;
                self.host.request_callback();
            }
            Err(error) => self.message = Some(error),
        }
    }
    fn draft_history_action(&mut self, redo: bool) {
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let current = edit.draft.state();
        if let Some(restored) = edit.history.step(current, redo) {
            edit.draft.restore_state(restored);
            edit.count = edit.draft.degrees.len();
            edit.page = edit
                .page
                .min(edit.draft.degrees.len().div_ceil(24).saturating_sub(1));
            edit.error = None;
        }
    }
    fn draw_history_controls(
        ui: &mut egui::Ui,
        p: Palette,
        undo: Option<String>,
        redo: Option<String>,
        ready: bool,
        trace_names: [&str; 2],
    ) -> Option<bool> {
        let (r, _) = ui.allocate_exact_size(vec2(48., 28.), Sense::hover());
        let mut selected = None;
        for (index, (label, redo, name, trace_name)) in [
            (undo, false, "Undo", trace_names[0]),
            (redo, true, "Redo", trace_names[1]),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = egui::Rect::from_center_size(
                pos2(r.left() + 11.5 + index as f32 * 25., r.center().y),
                vec2(23., 23.),
            );
            let enabled = ready && label.is_some();
            let response = ui.interact(
                rect,
                ui.id().with(trace_name),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            #[cfg(test)]
            trace(ui, trace_name, rect);
            let tip = label.map_or(name.to_owned(), |label| format!("{name} · {label}"));
            let response = response.on_hover_text(tip);
            if enabled && response.hovered() {
                ui.painter().rect_filled(rect, 3., p.track);
            }
            let color = if enabled { p.muted } else { p.rule };
            let direction = if redo { 1. } else { -1. };
            let c = rect.center();
            let tip = pos2(c.x + direction * 6., c.y - 1.);
            let shoulder = tip.x - direction * 4.;
            let stroke = Stroke::new(1.2, color);
            ui.painter().line(
                vec![
                    tip,
                    pos2(c.x - direction * 4., c.y - 1.),
                    pos2(c.x - direction * 7., c.y + 2.),
                    pos2(c.x - direction * 7., c.y + 6.),
                ],
                stroke,
            );
            for y in [-3.5, 3.5] {
                ui.painter()
                    .line_segment([tip, pos2(shoulder, tip.y + y)], stroke);
            }
            if response.clicked() {
                selected = Some(redo);
            }
        }
        selected
    }
    fn history_controls(&mut self, ui: &mut egui::Ui, p: Palette) {
        let (undo, redo) = self.shared.history_labels();
        let ready = self.job.is_none() && self.queue.is_empty() && self.edit.is_none();
        if let Some(redo) = Self::draw_history_controls(ui, p, undo, redo, ready, ["Undo", "Redo"])
        {
            self.set_history_action(if redo { 1 } else { -1 });
        }
    }
    fn scale_panel(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        project: &Project,
        active: usize,
        name: &str,
        empty: bool,
    ) {
        let is_preview = self.inspection.library().is_some();
        let inspected = self.inspection.slot().unwrap_or(active);
        let scale = if is_preview {
            self.preview.clone()
        } else {
            self.shared.scale(inspected)
        };
        let detail = scale.as_ref().map(ValidatedScale::parts);
        let popup_was_open = egui::Popup::is_any_open(ui.ctx());
        let height = (ui.available_height() - 8.).max(200.);
        egui::Frame::new().fill(p.panel).inner_margin(egui::Margin {left: 0, right: 0, top: 8, bottom: 0}).show(ui,|ui|{
            ui.set_min_width(ui.available_width());


            ui.set_min_height(height);ui.set_max_height(height);
            if self.preferences.view.browser_open {
                let toolbar=ui.allocate_ui_with_layout(vec2(ui.available_width(),28.),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                    self.browser_tabs(ui,p);
                    ui.set_min_height(28.);
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x=2.;
                        for forward in [false,true] {
                            let target=crate::editor_model::adjacent_slot(project,active,forward);
                            let available=if self.browser_tab == BrowserTab::Set {target.is_some()}else{self.step_target(forward).is_some()};
                            let button=ui.add_enabled_ui(self.job.is_none() && available,|ui|small_icon(ui,if forward {"next"}else{"previous"},Icon::Chevron(forward),if self.browser_tab == BrowserTab::Set {if forward {"Set position · next occupied slot"}else{"Set position · previous occupied slot"}}else if forward {"Next library scale"}else{"Previous library scale"},p)).inner;
                            #[cfg(test)] trace(ui,if forward {"next-scale"}else{"previous-scale"},button.rect);
                            if button.clicked() {if self.browser_tab == BrowserTab::Set {if let Some(slot)=target {self.activate_slot(slot);}}else if let Some(id)=self.step_target(forward){self.choose_library_scale(id);}}
                        }
                        if self.browser_tab == BrowserTab::Set {
                            let (rect,response)=ui.allocate_exact_size(vec2(18.,23.),Sense::hover());
                            ui.painter().text(rect.center(),Align2::CENTER_CENTER,(active+1).to_string(),FontId::proportional(TEXT_BODY),if project.slots[active].is_some(){p.orange}else{p.muted});
                            response.on_hover_text("Set position · current automation slot");
                        }
                    });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(31.);
                self.history_controls(ui,p);
                if self.browser_tab == BrowserTab::Set {
                let mut morph = self.shared.parameters.read().unwrap_or_default().morph_ms;
                let response = ui.add(egui::DragValue::new(&mut morph).range(0.0..=10000.0).speed(10.).fixed_decimals(0).suffix(" ms"))
                    .on_hover_ui(|ui| {ui.set_width(220.);ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);ui.label("Transition time between tunings. 0 ms switches immediately. Instruments must support continuous retuning to follow a morph.");});
                #[cfg(test)] trace(ui,"set-morph",response.rect);
                if response.drag_started() { self.host.begin_parameter(Parameter::MorphTime); }
                if response.changed() {self.set_parameter(Parameter::MorphTime,morph);}
                ui.label(RichText::new("MORPH").small().color(p.muted));
                }
            });
                });
                let browser_area=ui.horizontal_top(|ui| {
                    let library_width=(ui.available_width()-160.).max(180.);
                    ui.allocate_ui_with_layout(vec2(library_width,height-34.),egui::Layout::top_down(egui::Align::Min),|ui| {
                        ui.set_width(library_width);
                        if self.browser_tab == BrowserTab::Set { self.set_panel(ui,p,project,active,height-34.); }
                        else {self.library_panel(ui,p,height-34.);}

                    });
                    let info=ui.allocate_ui_with_layout(vec2(ui.available_width(),height-34.),egui::Layout::top_down(egui::Align::Min),|ui| {
                        ui.add_space(if self.browser_tab == BrowserTab::Set {0.} else {4.});

                        ui.style_mut().spacing.scroll.floating=true;
                        ui.style_mut().spacing.scroll.floating_allocated_width=0.;
                        ui.style_mut().spacing.scroll.bar_width=4.;
                        let details_width=(ui.available_width()-6.).max(40.);
                        ScrollArea::vertical().id_salt("browser-details").min_scrolled_height(0.).max_height(height-38.).show(ui,|ui| {
                            ui.set_max_width(details_width);
                            ui.set_min_width(details_width);
                            if let Some((preset,t))=&detail {
                                if !is_preview && inspected != active {ui.label(RichText::new(format!("INACTIVE · SLOT {}",inspected+1)).small().color(p.muted));}
                                let mut job=egui::text::LayoutJob::simple(preset.display_name.clone(),FontId::proportional(TEXT_HEADING),p.ink,details_width);
                                job.wrap.max_rows=2;
                                let galley=ui.fonts_mut(|fonts|fonts.layout_job(job));
                                let title=ui.label(galley).on_hover_text(&preset.display_name);
                                #[cfg(test)] trace(ui,"browser-name",title.rect);
                                #[cfg(not(test))] let _=title;
                                let shape=ScaleShape::from_prepared(t);
                                draw_scale_sized(ui,&shape,p,56.);
                                ui.label(RichText::new(shape.summary()).small().color(p.orange)).on_hover_text(shape.period_detail());
                                ui.label(RichText::new(crate::editor_model::mapping_summary(preset,t)).small().color(p.muted));
                                self.playing_ideas(ui,p,crate::listening::hint_for(preset,t));
                            } else {
                                if let Some(error) = &self.preview_error {
                                    ui.label(RichText::new("Could not load scale").small());
                                    ui.label(RichText::new(error).small());
                                } else {
                                    ui.label(
                                        RichText::new(if is_preview { "Loading scale…" } else { name })
                                            .small(),
                                    );
                                }
                            }
                        });
                    });
                    #[cfg(test)] trace(ui,"browser-info",info.response.rect);
                    #[cfg(not(test))] let _=info;
                });
                if !popup_was_open && self.overlay == Overlay::None && !egui::Popup::is_any_open(ui.ctx()) && !egui::DragAndDrop::has_any_payload(ui.ctx()) && ui.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|pos| !browser_area.response.rect.union(toolbar.response.rect).contains(pos))) {
                    self.set_view(ViewPreferences{browser_open:false,..self.preferences.view});
                }
                return;
            }
            ui.allocate_ui_with_layout(vec2(ui.available_width(),28.),egui::Layout::left_to_right(egui::Align::Center),|ui|{
                self.browser_tabs(ui,p);
                let title=detail.as_ref().map_or(name,|(preset,_)|preset.display_name.as_str());
                ui.allocate_ui_with_layout(vec2((ui.available_width()-88.).max(30.),28.),egui::Layout::left_to_right(egui::Align::Center),|ui| {ui.set_min_width(ui.available_width());ui.add(egui::Label::new(RichText::new(title).size(TEXT_TITLE).color(p.ink)).truncate()).on_hover_text(title);});
                self.history_controls(ui,p);
                    if detail.is_some() {
                        let button=ui.add_enabled_ui(self.job.is_none(), |ui| icon(ui,"edit-scale",Icon::Gear,"Edit this scale",p)).inner;
                        #[cfg(test)] trace(ui,"edit-scale",button.rect);
                        if button.clicked() && let Err(error) = self.edit_selected() {
                            self.message = Some(error);
                        }
                    }
            });
            ui.add_space(((height - 34. - 190.) / 2.).max(0.));
            ui.horizontal_top(|ui|{
                ui.add_space(12.);
                if let Some((_,t))=&detail {draw_scale_compared(ui,&ScaleShape::from_prepared(t),p,160.,self.reference_ring);}
                ui.add_space(6.);
                ui.vertical(|ui|{
                    let info_id=ui.id().with("scale-info-height");
                    let measured=ui.ctx().data(|d|d.get_temp::<f32>(info_id)).unwrap_or(70.);
                    ui.add_space(((160.-measured)/2.).max(0.));
                    let info_top=ui.cursor().top();
                    ui.vertical(|ui| {
                    if let Some((preset,t))=&detail {
                        if is_preview || inspected != active {
                            ui.horizontal(|ui| {
                                if ui.small_button("Show active").clicked(){self.shared.stop_audition();self.inspection = Inspection::Active;self.preview=None;self.assignment = AssignmentIntent::Browse;}
                                if is_preview {ui.label(RichText::new(if self.shared.audition_enabled.load(SeqCst) {"AUDITION"}else{"PREVIEW"}).small().color(p.muted));}
                            });
                        }
                        if !is_preview && inspected != active {ui.label(RichText::new(format!("INACTIVE · SLOT {}",inspected+1)).small().color(p.muted));}
                        let shape=ScaleShape::from_prepared(t);
                        let summary=ui.label(RichText::new(shape.summary()).color(p.orange)).on_hover_text(shape.period_detail());
                        #[cfg(test)] trace(ui,"scale-summary",summary.rect);
                        #[cfg(not(test))] let _ = summary;
                        let description = crate::listening::description(preset);
                        let description_scroll=ScrollArea::vertical().id_salt("scale-description").min_scrolled_height(0.).max_height(46.).show(ui,|ui|{if !description.is_empty() {ui.label(description);}});
                        #[cfg(test)] trace(ui,"scale-description",description_scroll.inner_rect);
                        #[cfg(not(test))] let _=description_scroll;
                        ui.label(RichText::new(crate::editor_model::mapping_summary(preset,t)).size(TEXT_BODY).color(p.muted)).on_hover_text(format!("Each consecutive MIDI key advances one scale degree unless a custom keyboard mapping changes it. Piano-key names do not indicate the resulting pitches. Frequency reference: MIDI {} at {:.3} Hz in the source mapping; the footer sets its current frequency.",t.reference_note,t.original_reference));
                        if !is_preview && inspected==active && project.slots.iter().flatten().count()>1 {
                            let last=(0..32).rfind(|&n|project.has_slot(n)).unwrap_or(active)+1;
                            let context=ui.label(RichText::new(format!("Scale set · {} of {}",active+1,last)).small().color(p.muted));
                            #[cfg(test)] trace(ui,"set-context",context.rect);
                            #[cfg(not(test))] let _=context;
                        }
                        let hint=crate::listening::hint_for(preset,t);
                        self.playing_ideas(ui,p,hint);
                        ui.ctx().data_mut(|d|d.insert_temp(info_id,ui.cursor().top()-info_top));
                    }else if is_preview && self.preview_error.is_some() {
                        ui.label(RichText::new("Could not load scale").size(TEXT_TITLE));
                        ScrollArea::vertical().id_salt("scale-load-error").max_height(90.).show(ui, |ui| { ui.label(self.preview_error.as_deref().unwrap_or_default()); });
                    }else{if !is_preview {ui.label(RichText::new(format!("UNASSIGNED · SLOT {}",inspected+1)).small().color(p.muted));}ui.label(RichText::new(if is_preview{"Loading scale…"}else if inspected != active {"Empty slot"}else{name}).size(TEXT_TITLE));if !is_preview {if inspected == active && empty {ui.label("Empty slot · holding the previous tuning");} else if inspected != active {ui.label("Choose a scale from the Library for this slot.");}}}
                    });

                });
            });
        });
    }
    fn playing_ideas(&mut self, ui: &mut egui::Ui, p: Palette, hint: &str) {
        if !hint.is_empty() {
            let ideas = ui.small_button(
                RichText::new("Playing ideas")
                    .size(TEXT_BODY)
                    .color(p.muted),
            );
            #[cfg(test)]
            trace(ui, "listening-ideas", ideas.rect);
            if ideas.clicked() {
                self.preferences.view.listening_ideas = !self.preferences.view.listening_ideas;
            }
            egui::Popup::menu(&ideas)
                .open_bool(&mut self.preferences.view.listening_ideas)
                .show(|ui| {
                    ui.set_width(240.);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    let text = ui.label(RichText::new(hint).size(TEXT_BODY));
                    #[cfg(test)]
                    trace(ui, "scale-description", text.rect);
                    #[cfg(not(test))]
                    let _ = text;
                });
        }
    }
    pub fn show_reference_ring(&mut self) {
        self.reference_ring = true;
    }
    fn show_connection_details(&mut self, ctx: &egui::Context) {
        let response=egui::Modal::new(egui::Id::new("mts-connection-details")).show(ctx,|ui| {
            ui.set_width(400.);
            ui.horizontal(|ui| {
                ui.strong("Inton is not sending tuning.");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    if icon(ui,"close-connection",Icon::Close,"Cancel (Escape)",Palette::new(self.preferences.view.dark)).clicked() {ui.close();}
                });
            });
            let blocked=self.shared.status.lock().unwrap().0 == MasterStatus::BlockedByOtherMaster;
            let recovery=blocked && self.shared.recovery_available.load(SeqCst);
            if blocked {
                ui.label("MTS reports another tuning master. A plugin crash or sandbox restart can also leave this connection behind.");
                if recovery {
                    ui.label("Reset only if no other tuning master is running. This clears the shared tuning and client registrations, then reconnects Inton with its current scale.");
                    ui.label("After resetting, disable all receiving instruments before enabling them again, or reload them together, to rebuild the client count.");
                } else {ui.label("Reset is unavailable for this connection. Close or disable the other tuning master to let Inton connect.");}
            } else {ui.label("The connection status has changed. Close this panel to check the footer.");}
            ui.horizontal(|ui| {
                if recovery {
                    let reset=ui.button("Reset and reconnect");
                    #[cfg(test)] trace(ui,"recover-connection",reset.rect);
                    if reset.clicked() {self.shared.recovery_requested.store(true,SeqCst);ui.close();}
                }
                let cancel=ui.button("Cancel");
                #[cfg(test)] trace(ui,"cancel-connection",cancel.rect);
                if cancel.clicked() {ui.close();}
            });
        });
        if response.should_close() {
            self.overlay = Overlay::None;
        }
    }
    fn browse_guided_scales(&mut self) {
        self.section = 1;
        self.category = "Listening guide".into();
        self.search.clear();
        self.library_scroll = 0.;
        self.browser_cursor = None;
        if !self.preferences.view.browser_open {
            self.set_view(ViewPreferences {
                browser_open: true,
                ..self.preferences.view
            });
        }
    }
    fn show_listening_guide(&mut self, ctx: &egui::Context) {
        let response = egui::Modal::new(egui::Id::new("listening-guide-panel")).show(ctx, |ui| {
            // Leave room for the modal frame inside the plugin window.
            ui.set_width((ctx.content_rect().width() - 32.).clamp(100., 420.));
            ui.horizontal(|ui| {
                ui.strong(if self.overlay == Overlay::QuickGuide {
                    "Quick guide"
                } else {
                    "Listening guide"
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon(
                        ui,
                        "close-guide",
                        Icon::Close,
                        "Close (Escape)",
                        Palette::new(self.preferences.view.dark),
                    )
                    .clicked()
                    {
                        ui.close();
                    }
                });
            });
            if self.overlay != Overlay::QuickGuide {
                let browse = ui.button("Browse guided scales");
                #[cfg(test)]
                trace(ui, "browse-guided-scales", browse.rect);
                if browse.clicked() {
                    self.browse_guided_scales();
                    ui.close();
                }
            }
            ui.separator();
            ScrollArea::vertical()
                .id_salt("listening-guide-content")
                .max_height(320.)
                .show(ui, |ui| {
                    // Guides use headings, paragraphs and standalone source links.
                    // Render the same embedded text, so the two editions stay in sync.
                    for block in (if self.overlay == Overlay::QuickGuide {
                        include_str!("../docs/quick-guide.md")
                    } else {
                        include_str!("../docs/first-scales.md")
                    })
                    .trim()
                    .split("\n\n")
                    {
                        if let Some(title) = block.strip_prefix("# ") {
                            ui.heading(title);
                        } else if let Some(title) = block.strip_prefix("## ") {
                            ui.strong(title);
                        } else if let Some((label, url)) = block
                            .strip_prefix('[')
                            .and_then(|s| s.strip_suffix(')'))
                            .and_then(|s| s.split_once("]("))
                        {
                            if ui.link(label).on_hover_text(url).clicked() {
                                let url = url.to_string();
                                thread::spawn(move || {
                                    let _ =
                                        std::process::Command::new("xdg-open").arg(url).status();
                                });
                            }
                        } else {
                            ui.label(block);
                        }
                        ui.add_space(6.);
                    }
                });
        });
        if response.should_close() {
            self.overlay = Overlay::None;
        }
    }
    fn folder_settings(&mut self, ctx: &egui::Context) {
        let response = egui::Modal::new(egui::Id::new("library-folder-settings")).show(ctx, |ui| {
            ui.set_width(350.);
            ui.horizontal(|ui| {
                ui.strong("Library folder");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon(
                        ui,
                        "close-settings",
                        Icon::Close,
                        "Close (Escape)",
                        Palette::new(self.preferences.view.dark),
                    )
                    .clicked()
                    {
                        ui.close();
                    }
                });
            });
            ui.label("User scales and matching keyboard maps");
            let folder =
                ui.add(egui::TextEdit::singleline(&mut self.folder_text).desired_width(350.));
            #[cfg(test)]
            trace(ui, "folder-path", folder.rect);
            let _ = folder;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.job.is_none(), egui::Button::new("Apply folder"))
                    .clicked()
                {
                    let path = PathBuf::from(&self.folder_text);
                    if path.is_absolute() {
                        self.preferences.folder = path;
                        self.save_preferences();
                        self.enqueue(Command::Rescan);
                    } else {
                        self.message = Some("Use an absolute folder path.".into());
                    }
                }
                if ui.button("Open folder").clicked() {
                    let folder = self.preferences.folder.clone();
                    thread::spawn(move || {
                        let _ = std::process::Command::new("xdg-open").arg(folder).status();
                    });
                }
            });
        });
        if response.should_close() {
            self.overlay = Overlay::None;
        }
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.shared.stop_audition();
    }
}
impl NiceEguiApp for Editor {
    fn build(
        &mut self,
        ctx: egui::Context,
        gui_context: GuiContext,
        _: &mut Frame,
    ) -> Result<(), HandlerError> {
        self.host.attach(gui_context);
        oiko_ui::scale::initialize_scale(
            &ctx,
            self.preferences.view.scale as f32,
            vec2(
                self.preferences.view.width() as f32,
                self.preferences.view.height() as f32,
            ),
        );
        Ok(())
    }
    fn editor_closed(&mut self) {
        self.host.detach();
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame) {
        let (undo, redo) = self.shared.history_labels();
        frame.set_key_capture(host_key_capture(
            self.edit.is_some() || undo.is_some() || redo.is_some(),
            ui.ctx().text_edit_focused(),
        ));
        #[cfg(target_os = "macos")]
        {
            // Match Weft's AppKit path: the host window is sized in logical points,
            // egui stays at 1x, and the user's interface scale transforms a fixed canvas.
            let scale = ViewPreferences::nearest_scale(self.preferences.view.scale) as f32;
            let content_layer =
                egui::LayerId::new(egui::Order::Middle, egui::Id::new("inton-scaled-content"));
            ui.ctx()
                .set_transform_layer(content_layer, egui::emath::TSTransform::from_scaling(scale));
            let content_rect = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(
                    self.preferences.view.width() as f32,
                    self.preferences.view.height() as f32,
                ),
            );
            let mut content_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("inton-scaled-content")
                    .layer_id(content_layer)
                    .max_rect(content_rect)
                    .layout(*ui.layout()),
            );
            content_ui.set_clip_rect(content_rect);
            content_ui.set_min_size(content_rect.size());
            self.draw(&mut content_ui);
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.draw(ui);
        }
        if !ui.input(|input| input.pointer.any_down()) || !ui.input(|input| input.focused) {
            self.host.finish_gestures();
        }
        let canvas = vec2(
            self.preferences.view.width() as f32,
            self.preferences.view.height() as f32,
        );
        if let Some(scale) =
            oiko_ui::resize_grip::show(ui, canvas, self.preferences.view.scale as f32)
        {
            self.set_view(ViewPreferences {
                scale: f64::from(scale),
                ..self.preferences.view
            });
            oiko_ui::scale::request_scale(ui.ctx(), scale, canvas);
        }
    }
}
fn host_key_capture(history_active: bool, text_edit_focused: bool) -> KeyCapture {
    if text_edit_focused {
        KeyCapture::CaptureAll
    } else if history_active {
        KeyCapture::CaptureCommands(vec![
            HostKey::Character("z".into()),
            HostKey::Character("Z".into()),
        ])
    } else {
        KeyCapture::IgnoreAll
    }
}
fn file_dialog(folder: bool) -> Result<Option<PathBuf>, String> {
    let mut zenity = std::process::Command::new("zenity");

    zenity.args(["--file-selection", "--title=Oiko Inton — Add Scale"]);

    if folder {
        zenity.arg("--directory");
    } else {
        zenity.arg("--file-filter=Scala scales | *.scl *.SCL");
    }

    let output=match zenity.output(){
Ok(o)=>o,Err(_)=>std::process::Command::new("kdialog").args(if folder{
vec!["--getexistingdirectory","."]}
else{
vec!["--getopenfilename",".","*.scl *.SCL|Scala scales"]}
).output().map_err(|_|"File dialog unavailable. Copy .scl and matching .kbm files into the displayed User Scale Folder, then Rescan.")?}
;

    dialog_selection(output.status.code(), output.stdout)
}

fn dialog_selection(code: Option<i32>, stdout: Vec<u8>) -> Result<Option<PathBuf>, String> {
    if code == Some(1) {
        return Ok(None);
    }
    if code != Some(0) {
        return Err("File dialog failed. Copy .scl and matching .kbm files into the User Scale Folder, then Rescan.".into());
    }

    let path = String::from_utf8(stdout).map_err(|e| e.to_string())?;

    let path = PathBuf::from(path.trim());

    if path.as_os_str().is_empty() {
        return Ok(None);
    }

    Ok(Some(path))
}
#[cfg(test)]
fn trace(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(key), rect));
}

#[cfg(test)]
mod tests;
