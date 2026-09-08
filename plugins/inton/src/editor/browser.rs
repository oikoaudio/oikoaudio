//! Library browsing, filtering and assignment interactions.
use super::rendering::{Icon, small_icon};
#[cfg(test)]
use super::trace;
use super::{Command, Editor, ScaleDrag};
use crate::editor_model::{AssignmentIntent, BrowserTab, Inspection, Overlay, ViewPreferences};
use crate::editor_theme::{Palette, TEXT_SMALL, compact_rows};
use crate::library::Library;
use egui::{FontId, RichText, ScrollArea, Sense, Stroke, vec2};
use egui::{Vec2, pos2};
use inton_core::engine::Parameter;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

impl Editor {
    pub(super) fn filtered_scales<'a>(&self, lib: &'a Library) -> Vec<&'a crate::library::Entry> {
        let mut rows: Vec<_> = lib
            .search(&self.search)
            .into_iter()
            .filter(|e| {
                (self.category == "All categories"
                    || e.category == self.category
                    || (self.category == "Listening guide"
                        && e.tags.iter().any(|t| t == "listening-guide")))
                    && match self.section {
                        0 => self.preferences.favorites.contains(&e.id),
                        1 => e.path.is_none(),
                        2 => e.path.is_some(),
                        _ => true,
                    }
            })
            .collect();
        if self.category == "Listening guide" {
            rows.sort_by_key(|e| e.tags.iter().find(|t| t.starts_with("lesson:")));
        }
        rows
    }
    pub(super) fn step_target(&self, forward: bool) -> Option<String> {
        let lib = self.library.as_ref()?;
        let rows = self.filtered_scales(lib);
        if rows.is_empty() {
            return None;
        }
        let project = self.shared.snapshot();
        let slot = self
            .inspection
            .slot()
            .unwrap_or(project.parameters.position);
        let current = self
            .browser_cursor
            .as_deref()
            .or(self.inspection.library().map(String::as_str))
            .or_else(|| {
                project.slots[slot]
                    .as_ref()
                    .and_then(|p| p.source_id.as_deref())
            });
        let index = current.and_then(|id| rows.iter().position(|e| e.id == id));
        let next = match index {
            Some(n) if forward => n.checked_add(1).filter(|n| *n < rows.len())?,
            Some(n) => n.checked_sub(1)?,
            None if forward => 0,
            None => rows.len() - 1,
        };
        Some(rows[next].id.clone())
    }
    #[cfg(test)]
    pub(super) fn step_scale(&mut self, forward: bool) {
        let project = self.shared.snapshot();
        if project.slots.iter().flatten().count() > 1 {
            if let Some(slot) =
                crate::editor_model::adjacent_slot(&project, project.parameters.position, forward)
            {
                self.activate_slot(slot);
            }
        } else if let Some(id) = self.step_target(forward) {
            let slot = project.slots.iter().position(Option::is_some).unwrap_or(0);
            self.shared.stop_audition();
            self.inspection = Inspection::Active;
            self.preview = None;
            self.browser_cursor = Some(id.clone());
            self.assign_scale(id, slot);
            if project.parameters.position != slot {
                self.set_parameter(Parameter::Position, slot as f64);
            }
        }
    }
    pub(super) fn direct_library_load(&self) -> bool {
        let project = self.shared.snapshot();
        project.slots.iter().flatten().count() <= 1
            && self.assignment != AssignmentIntent::Append
            && project.slots[self.set.destination].is_some()
    }
    pub(super) fn choose_library_scale(&mut self, id: String) {
        self.library_reveal = Some(id.clone());
        if self.direct_library_load() {
            self.shared.stop_audition();
            self.browser_cursor = Some(id.clone());
            self.assign_scale(id, self.set.destination);
        } else {
            self.select(id);
        }
    }
    pub(super) fn focus_current_library(&mut self) {
        self.focus_library = true;
        let project = self.shared.snapshot();
        let current = self.inspection.library().cloned().or_else(|| {
            project.slots[project.parameters.position]
                .as_ref()
                .and_then(|preset| preset.source_id.clone())
        });
        if let Some(id) = current
            && let Some(lib) = &self.library
            && lib.entries.iter().any(|entry| entry.id == id)
        {
            if !self.filtered_scales(lib).iter().any(|entry| entry.id == id) {
                self.search.clear();
                self.category = "All categories".into();
                self.section = 3;
            }
            self.browser_cursor = Some(id.clone());
            self.library_reveal = Some(id);
        }
    }
    pub(super) fn browser_tabs(&mut self, ui: &mut egui::Ui, p: Palette) -> bool {
        let open = self.preferences.view.browser_open;
        let mut navigated = false;
        let library = scope_button(
            ui,
            "",
            false,
            open && self.browser_tab != BrowserTab::Set,
            p,
        )
        .on_hover_text(if open && self.browser_tab != BrowserTab::Set {
            "Close Library (Esc)"
        } else {
            "Open Library"
        });
        #[cfg(test)]
        {
            trace(ui, "library-tab", library.rect);
            trace(ui, "browse-chevron", library.rect);
        }
        if library.clicked() {
            navigated = true;
            if !open || self.browser_tab == BrowserTab::Set {
                self.focus_current_library();
            }
            self.set_view(ViewPreferences {
                browser_open: !open || self.browser_tab == BrowserTab::Set,
                ..self.preferences.view
            });
            self.browser_tab = BrowserTab::Library;
        }
        let project = scope_button(ui, "", true, open && self.browser_tab == BrowserTab::Set, p)
            .on_hover_text(if open && self.browser_tab == BrowserTab::Set {
                "Close scale set (Esc)"
            } else {
                "Open scale set · drop a scale here to append it"
            });
        #[cfg(test)]
        trace(ui, "project-tab", project.rect);
        let drop_hover = project.dnd_hover_payload::<ScaleDrag>().is_some();
        let flash = self
            .acknowledgment
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() < Duration::from_millis(700));
        if drop_hover || flash {
            ui.painter()
                .rect_filled(project.rect, 3, p.orange.gamma_multiply(0.18));
            ui.painter().rect_stroke(
                project.rect,
                3,
                Stroke::new(1.5, p.orange),
                egui::StrokeKind::Inside,
            );
        }
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("project-drop-hover"), drop_hover));
        if project.clicked() {
            navigated = true;
            if !open || self.browser_tab != BrowserTab::Set {
                self.follow_active_scale();
            }
            self.set_view(ViewPreferences {
                browser_open: !open || self.browser_tab != BrowserTab::Set,
                ..self.preferences.view
            });
            self.browser_tab = BrowserTab::Set;
        }
        if let Some(payload) = project.dnd_release_payload::<ScaleDrag>() {
            self.enqueue(Command::Append(payload.0.clone()));
        }
        navigated
    }
    pub(super) fn category_filter(&mut self, ui: &mut egui::Ui) {
        if let Some(lib) = self.library.clone() {
            let mut categories: Vec<_> = lib
                .entries
                .iter()
                .filter(|e| match self.section {
                    1 => e.path.is_none(),
                    2 => e.path.is_some(),
                    _ => true,
                })
                .map(|e| e.category.clone())
                .collect();
            categories.sort();
            categories.dedup();
            let filtered = self.category != "All categories";
            let orange = Palette::new(self.preferences.view.dark).orange;
            let category_menu = ui
                .scope(|ui| {
                    ui.spacing_mut().scroll.floating = false;
                    ui.spacing_mut().scroll.bar_width = 6.;
                    egui::ComboBox::from_id_salt("category")
                        .selected_text("")
                        .width(20.)
                        .icon(move |ui, rect, visuals, _| {
                            let c = rect.center();
                            let points = [
                                vec2(-5., -4.),
                                vec2(5., -4.),
                                vec2(1., 0.),
                                vec2(1., 5.),
                                vec2(-1., 4.),
                                vec2(-1., 0.),
                                vec2(-5., -4.),
                            ]
                            .into_iter()
                            .map(|v| c + v * 0.8)
                            .collect();
                            ui.painter().add(egui::Shape::line(
                                points,
                                if filtered {
                                    Stroke::new(1.2, orange)
                                } else {
                                    visuals.fg_stroke
                                },
                            ));
                        })
                        .show_ui(ui, |ui| {
                            compact_rows(ui);
                            ui.set_width(260.);
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                            ui.spacing_mut().scroll.floating = false;
                            ui.spacing_mut().scroll.bar_width = 6.;
                            ui.selectable_value(
                                &mut self.category,
                                "All categories".into(),
                                "All categories",
                            );
                            let guided = ui.selectable_label(
                                self.category == "Listening guide",
                                "Listening guide",
                            );
                            #[cfg(test)]
                            trace(ui, "guided-category", guided.rect);
                            if guided.clicked() {
                                self.browse_guided_scales();
                            }
                            for c in categories {
                                let choice = ui.selectable_value(
                                    &mut self.category,
                                    c.clone(),
                                    c.replace('/', " › "),
                                );
                                #[cfg(test)]
                                trace(ui, &format!("category-option:{c}"), choice.rect);
                                #[cfg(not(test))]
                                let _ = choice;
                            }
                        })
                })
                .inner;
            category_menu
                .response
                .clone()
                .on_hover_text(format!("Category filter: {}", self.category));
            #[cfg(test)]
            trace(ui, "category-menu", category_menu.response.rect);
            #[cfg(not(test))]
            let _ = category_menu;
        }
    }
    pub(super) fn library_panel(&mut self, ui: &mut egui::Ui, p: Palette, height: f32) {
        let previous_style = ui.style().clone();
        let panel_top = ui.cursor().top();
        let search_id = ui.make_persistent_id("library-search-input");
        let handoff = if ui.memory(|m| m.has_focus(search_id))
            && self.overlay == Overlay::None
            && !egui::Popup::is_any_open(ui.ctx())
        {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                Some(true)
            } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                Some(false)
            } else {
                None
            }
        } else {
            None
        };
        if handoff.is_some() {
            ui.memory_mut(|m| m.surrender_focus(search_id));
        }
        compact_rows(ui);
        ui.horizontal(|ui| {
            let search_response = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .id(search_id)
                    .hint_text("Search…")
                    .desired_width(ui.available_width()),
            );
            if self.focus_library {
                search_response.request_focus();
                self.focus_library = false;
            }
            #[cfg(test)]
            trace(ui, "library-search", search_response.rect);
            #[cfg(not(test))]
            let _ = search_response;
        });
        if matches!(self.assignment, AssignmentIntent::Replace(_)) {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Choose for Slot {}", self.set.destination + 1))
                        .small()
                        .color(p.orange),
                );
                if ui.small_button("Cancel").clicked()
                    || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                {
                    self.assignment = AssignmentIntent::Browse;
                }
            });
        }
        let append_intent = self.library_will_append();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.;
            ui.style_mut().override_font_id=Some(FontId::proportional(TEXT_SMALL));
            self.category_filter(ui);
            for (i, s) in ["Favorites", "Factory", "User"].iter().enumerate() {
                let filter=library_filter_button(ui,i,self.section==i,p).on_hover_text(*s);
                #[cfg(test)] trace(ui,&format!("source-filter:{i}"),filter.rect);
                if filter.clicked() {
                    self.section = if self.section == i {3}else{i};
                    self.category = "All categories".into();
                }
            }
            if self.shared.snapshot().slots.iter().flatten().count()>1 || self.assignment == AssignmentIntent::Append {
            let audition = self.shared.audition_enabled.load(SeqCst);
            let speaker = small_icon(ui,"audition",Icon::Speaker(audition),"Audition selected scales · click to toggle. Turning off restores the project tuning.",p);
            #[cfg(test)] trace(ui,"library-audition",speaker.rect);
            if speaker.clicked() {
                if audition {self.shared.stop_audition();} else {
                    self.shared.audition_enabled.store(true,SeqCst);
                    if let Some(scale)=&self.preview {self.shared.audition_prepared(scale.clone());}
                }
            }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x=8.;
                let settings = small_icon(ui, "settings", Icon::Gear, "Folder settings", p);
                #[cfg(test)]
                trace(ui, "folder-settings", settings.rect);
                if settings.clicked() {
                    self.overlay = Overlay::Settings;
                }
                ui.add_enabled_ui(self.job.is_none(), |ui| {
                    if small_icon(ui, "rescan", Icon::Refresh, "Rescan library", p).clicked() {
                        self.enqueue(Command::Rescan);
                    }
                    if small_icon(ui, "import", Icon::Plus, "Add Scale", p).clicked() {
                        self.enqueue(Command::Import);
                    }
                });
            });
        });
        let mut pick = None;
        let mut assign = None;
        let mut favorite = None;
        if let Some(lib) = self.library.clone() {
            let rows = self.filtered_scales(&lib);
            let count = rows.len();
            let mut target = self
                .library_reveal
                .take()
                .and_then(|id| rows.iter().position(|row| row.id == id));
            if self.overlay == Overlay::None
                && self.edit.is_none()
                && !ui.ctx().text_edit_focused()
                && !egui::Popup::is_any_open(ui.ctx())
                && !egui::DragAndDrop::has_any_payload(ui.ctx())
            {
                let up = handoff == Some(false)
                    || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                let down = handoff == Some(true)
                    || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                if (up || down) && count > 0 {
                    let current = rows
                        .iter()
                        .position(|r| Some(&r.id) == self.browser_cursor.as_ref());
                    let n = match current {
                        Some(n) if up => n.saturating_sub(1),
                        Some(n) => (n + 1).min(count - 1),
                        None => 0,
                    };
                    self.browser_cursor = Some(rows[n].id.clone());
                    pick = Some(rows[n].id.clone());
                    target = Some(n);
                }
                if handoff.is_none()
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
                    && let Some(row) = rows
                        .iter()
                        .find(|r| Some(&r.id) == self.browser_cursor.as_ref())
                {
                    assign = Some((row.id.clone(), self.set.destination));
                }
            }
            let list_height = (height - (ui.cursor().top() - panel_top)).max(40.);
            let mut scroll = ScrollArea::vertical()
                .id_salt("library")
                .max_height(list_height)
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);
            if let Some(n) = target {
                let stride = 17. + ui.spacing().item_spacing.y;
                let top = n as f32 * stride;
                let bottom = top + stride;
                let offset = if top < self.library_scroll {
                    top
                } else if bottom > self.library_scroll + list_height {
                    bottom - (list_height)
                } else {
                    self.library_scroll
                };
                scroll = scroll.vertical_scroll_offset(offset.max(0.));
            }
            ui.spacing_mut().interact_size.y = 17.;
            let output = scroll.show_rows(ui, 17., count, |ui, range| {
                for n in range {
                    let e = rows[n];
                    let error = lib.cached_error(&e.id);
                    ui.push_id(&e.id, |ui| {
                        ui.horizontal(|ui| {
                            let fav = self.preferences.favorites.contains(&e.id);
                            if small_icon(ui, "favorite", Icon::Star(fav), "Toggle favorite", p)
                                .clicked()
                            {
                                favorite = Some((e.id.clone(), !fav));
                            }
                            let selected = self.inspection.library() == Some(&e.id);
                            let highlighted = self.browser_cursor.as_ref() == Some(&e.id);
                            let width =
                                (ui.available_width() - if selected { 64. } else { 0. }).max(20.);
                            let r = ui
                                .add_sized(
                                    [width, 17.],
                                    egui::Button::new(
                                        RichText::new(if error.is_some() {
                                            format!("Invalid · {}", e.name)
                                        } else {
                                            e.name.clone()
                                        })
                                        .size(TEXT_SMALL)
                                        .color(
                                            if error.is_some() || highlighted {
                                                p.orange
                                            } else {
                                                p.ink
                                            },
                                        ),
                                    )
                                    .frame(false)
                                    .right_text("")
                                    .selected(highlighted)
                                    .truncate()
                                    .sense(Sense::click_and_drag()),
                                )
                                .on_hover_text(if let Some(error) = &error {
                                    format!("{}\n{}", e.name, error)
                                } else {
                                    e.name.clone()
                                });
                            #[cfg(test)]
                            trace(ui, &format!("scale:{}", e.id), r.rect);
                            #[cfg(test)]
                            if highlighted {
                                trace(ui, "library-selected", r.rect);
                            }
                            if handoff.is_some() && highlighted {
                                r.request_focus();
                            }
                            if r.clicked() {
                                pick = Some(e.id.clone());
                            }
                            if r.double_clicked() && error.is_none() {
                                assign = Some((e.id.clone(), self.set.destination));
                            }
                            if error.is_none() {
                                r.dnd_set_drag_payload(ScaleDrag(e.id.clone()));
                            }
                            if selected {
                                let action = ui
                                    .add_enabled(
                                        error.is_none(),
                                        egui::Button::new(if append_intent {
                                            "Add"
                                        } else {
                                            "Load"
                                        })
                                        .frame(true),
                                    )
                                    .on_hover_text(if append_intent {
                                        "Add to scale set".into()
                                    } else {
                                        if self.shared.snapshot().uses_scale_set() {
                                            format!("Load into Slot {}", self.set.destination + 1)
                                        } else {
                                            "Load current tuning".into()
                                        }
                                    });
                                #[cfg(test)]
                                trace(ui, &format!("assign:{}", e.id), action.rect);
                                if action.clicked() {
                                    assign = Some((e.id.clone(), self.set.destination));
                                }
                            }
                        });
                    });
                }
            });
            #[cfg(test)]
            trace(ui, "library-visible", output.inner_rect);
            self.library_scroll = output.state.offset.y;
            if count == 0 {
                ui.label(RichText::new("No matching scales").small().color(p.muted));
            }
        } else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Reading library…");
            });
        }
        if let Some(id) = pick {
            self.choose_library_scale(id);
        }
        if let Some((id, slot)) = assign {
            self.assign_from_library(id, slot);
        }
        if let Some((id, add)) = favorite {
            if add {
                self.preferences.favorites.insert(id);
            } else {
                self.preferences.favorites.remove(&id);
            }
            self.save_preferences();
        }
        ui.set_style(previous_style);
    }
}

fn library_filter_button(
    ui: &mut egui::Ui,
    kind: usize,
    selected: bool,
    p: Palette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(23., 20.), Sense::click());
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, 3, if selected { p.blue } else { p.track });
    }
    let c = rect.center();
    let stroke = Stroke::new(1.2, if selected { p.ink } else { p.muted });
    let paint = ui.painter();
    match kind {
        0 => {
            let points = (0..10)
                .map(|i| {
                    c + Vec2::angled(
                        i as f32 * std::f32::consts::TAU / 10. - std::f32::consts::FRAC_PI_2,
                    ) * if i % 2 == 0 { 6. } else { 2.7 }
                })
                .collect();
            paint.add(egui::Shape::closed_line(points, stroke));
        }
        1 => {
            paint.add(egui::Shape::closed_line(
                vec![
                    c + vec2(-6., 5.),
                    c + vec2(-6., -5.),
                    c + vec2(-3., -5.),
                    c + vec2(-3., 0.),
                    c + vec2(1., -3.),
                    c + vec2(1., 0.),
                    c + vec2(6., -3.),
                    c + vec2(6., 5.),
                ],
                stroke,
            ));
            for x in [-3., 1., 4.] {
                paint.line_segment([c + vec2(x, 2.), c + vec2(x, 4.)], stroke);
            }
        }
        2 => {
            paint.circle_stroke(c + vec2(0., -3.), 2.5, stroke);
            paint.add(egui::Shape::line(
                vec![
                    c + vec2(-5., 6.),
                    c + vec2(-5., 3.),
                    c + vec2(-2., 1.),
                    c + vec2(2., 1.),
                    c + vec2(5., 3.),
                    c + vec2(5., 6.),
                ],
                stroke,
            ));
        }
        _ => unreachable!("Only Favorites, Factory and User have filter buttons"),
    }
    response
}
// Share the same measured icon/text alignment between scope tabs and navigation.
fn scope_button(
    ui: &mut egui::Ui,
    label: &str,
    project: bool,
    selected: bool,
    p: Palette,
) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(TEXT_SMALL), p.ink);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + 27., 23.), Sense::click());
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, 3, if selected { p.blue } else { p.track });
    }
    let c = pos2(rect.left() + 11., rect.center().y);
    let stroke = Stroke::new(1.1, if selected { p.ink } else { p.muted });
    if project {
        for y in [-4., 0., 4.] {
            ui.painter()
                .circle_filled(c + vec2(-5., y), 1., stroke.color);
            ui.painter()
                .line_segment([c + vec2(-2., y), c + vec2(5., y)], stroke);
        }
    } else {
        ui.painter().add(egui::Shape::line(
            vec![
                c + vec2(-6., 4.),
                c + vec2(-6., -5.),
                c + vec2(-2., -5.),
                c + vec2(0., -2.),
                c + vec2(6., -2.),
                c + vec2(6., 4.),
                c + vec2(-6., 4.),
            ],
            stroke,
        ));
        ui.painter()
            .line_segment([c + vec2(-6., -1.), c + vec2(6., -1.)], stroke);
    }
    ui.painter().galley(
        pos2(rect.left() + 22., rect.center().y - galley.size().y / 2.),
        galley,
        p.ink,
    );
    response
}
