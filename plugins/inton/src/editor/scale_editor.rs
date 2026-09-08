//! Scale drafts, audition and edit history.
use super::Editor;
use super::rendering::{Icon, draw_scale_sized, icon, small_icon};
#[cfg(test)]
use super::trace;
use crate::editor_model::ScaleShape;
use crate::editor_theme::{Palette, TEXT_SMALL, TEXT_TITLE, compact_rows};
use egui::{FontId, RichText, Stroke, vec2};
use std::sync::atomic::Ordering::SeqCst;
use std::time::{Duration, Instant};

impl Editor {
    pub fn edit_selected(&mut self) -> Result<(), String> {
        if self.job.is_some() {
            return Err("Wait for the scale to finish loading.".into());
        }
        let destination = if self.inspection.library().is_some() {
            self.set.destination
        } else {
            self.inspection
                .slot()
                .unwrap_or(self.shared.snapshot().parameters.position)
        };
        let scale = if self.inspection.library().is_some() {
            self.preview.clone()
        } else {
            self.shared.scale(destination)
        }
        .ok_or("Choose a scale to edit.")?;
        let draft = inton_core::scale_edit::Draft::from_validated(scale);
        let count = draft.degrees.len();
        self.shared.stop_audition();
        self.edit = Some(ScaleEdit {
            draft,
            destination,
            append: self.inspection.library().is_some() && self.library_will_append(),
            mode: 0,
            page: 0,
            count,
            audition: false,
            error: None,
            history: DraftHistory::default(),
        });
        Ok(())
    }
    pub(super) fn edit_panel(&mut self, ui: &mut egui::Ui, p: Palette, height: f32) {
        let mut apply = false;
        let mut cancel = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if self.edit.is_none() {
            return;
        }
        if !self.shared.audition_enabled.load(SeqCst) {
            self.edit.as_mut().unwrap().audition = false;
        }
        let before = self.edit.as_ref().unwrap().draft.state();
        let mut history_action = None;
        egui::Frame::new()
            .fill(p.panel)
            .corner_radius(5)
            .inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 8,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), 28.),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        cancel |= self.browser_tabs(ui, p);
                        let edit = self.edit.as_mut().unwrap();
                        ui.allocate_ui_with_layout(
                            vec2((ui.available_width() - 88.).max(30.), 28.),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.style_mut().text_styles.insert(
                                    egui::TextStyle::Body,
                                    FontId::proportional(TEXT_TITLE),
                                );
                                let response = ui
                                    .add(
                                        egui::TextEdit::singleline(&mut edit.draft.name)
                                            .desired_width(ui.available_width())
                                            .frame(egui::Frame::NONE)
                                            .text_color(p.ink),
                                    )
                                    .on_hover_text("Edit scale name");
                                if response.hovered() || response.has_focus() {
                                    ui.painter().line_segment(
                                        [response.rect.left_bottom(), response.rect.right_bottom()],
                                        Stroke::new(1., p.orange),
                                    );
                                }
                                #[cfg(test)]
                                trace(ui, "edit-name", response.rect);
                            },
                        );
                        history_action = Self::draw_history_controls(
                            ui,
                            p,
                            edit.history
                                .can_undo()
                                .then(|| "Draft edit · Cmd+Z".to_owned()),
                            edit.history
                                .can_redo()
                                .then(|| "Draft edit · Cmd+Shift+Z".to_owned()),
                            true,
                            ["edit-undo", "edit-redo"],
                        );
                        let close = icon(
                            ui,
                            "edit-scale",
                            Icon::Gear,
                            "Close scale editor without applying · Escape",
                            p,
                        );
                        #[cfg(test)]
                        trace(ui, "edit-scale", close.rect);
                        cancel |= close.clicked();
                    },
                );
            });
        let edit = self.edit.as_mut().unwrap();
        egui::Frame::new().fill(p.panel).inner_margin(0).show(ui, |ui| {
            compact_rows(ui);
            ui.set_min_height(height-36.);
            ui.spacing_mut().interact_size.y = 16.;
            ui.spacing_mut().button_padding.y = 0.;
            let note_count = edit.draft.degrees.len();
            // Eight rows leave a dependable action area above the fixed footer.
            // Each page is divided evenly across the three compact columns.
            let rows_per_column = 8;
            let column_count = 3;
            let notes_per_page = rows_per_column * column_count;
            const NOTE_INDEX_WIDTH: f32 = 14.;
            const NOTE_VALUE_WIDTH: f32 = 52.;
            const NOTE_BUTTON_WIDTH: f32 = 14.;
            const NOTE_ITEM_GAP: f32 = 3.;
            const GRID_RIGHT_PADDING: f32 = 6.;
            const NOTE_GROUP_WIDTH: f32 = NOTE_INDEX_WIDTH
                + NOTE_VALUE_WIDTH
                + NOTE_BUTTON_WIDTH * 2.
                + NOTE_ITEM_GAP * 3.;
            const INSERT_TAIL_WIDTH: f32 = NOTE_ITEM_GAP + NOTE_BUTTON_WIDTH;
            let grid_left_margin = GRID_RIGHT_PADDING;
            let column_gap = ((ui.available_width()
                - grid_left_margin
                - GRID_RIGHT_PADDING
                - NOTE_GROUP_WIDTH * column_count as f32)
                / (column_count - 1) as f32)
                .max(6.);
            let pages = note_count.div_ceil(notes_per_page).max(1);
                edit.page = edit.page.min(pages-1);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x=NOTE_ITEM_GAP;
                    ui.spacing_mut().button_padding.x=3.;
                    ui.add_space(grid_left_margin);
                    ui.add_sized([NOTE_INDEX_WIDTH,16.],egui::Label::new(""));
                    ui.add_sized([NOTE_VALUE_WIDTH,16.],egui::Label::new(format!("{note_count} notes"))).on_hover_text("Includes the root. The period is the repeat point, not another note.");
                    let subtract=ui.add_enabled(note_count>1,egui::Button::new("−").small().min_size(vec2(NOTE_BUTTON_WIDTH,15.))).on_hover_text("Remove the last note before the repeat point");
                    #[cfg(test)] trace(ui,"edit-remove-last-note",subtract.rect);
                    if subtract.clicked() {edit.error=edit.draft.remove_note(note_count-2).err();}
                    let add = ui.add_enabled(edit.draft.degrees.len() < 4096, egui::Button::new("+").small().min_size(vec2(NOTE_BUTTON_WIDTH,15.))).on_hover_text("Add a note before the repeat point");
                    #[cfg(test)] trace(ui,"edit-add-note",add.rect);
                    if add.clicked() {
                        edit.error = edit.draft.add_note().err();
                        edit.page = (edit.draft.degrees.len()-1)/notes_per_page;
                    }
                    if pages>1 {
                        if ui.add_enabled(edit.page>0,egui::Button::new("‹").small().min_size(vec2(NOTE_BUTTON_WIDTH,15.))).on_hover_text("Previous page").clicked() {edit.page-=1;}
                        ui.label(format!("{}/{}",edit.page+1,pages)).on_hover_text("Interval page");
                        if ui.add_enabled(edit.page+1<pages,egui::Button::new("›").small().min_size(vec2(NOTE_BUTTON_WIDTH,15.))).on_hover_text("Next page").clicked() {edit.page+=1;}
                    }
                    ui.add_space(6.);
                    ui.label("Period");
                    let last=edit.draft.degrees.len()-1;
                    let octaves=edit.draft.degrees[last]/1200.;
                    let period_label=if (octaves-octaves.round()).abs()<1e-8 {format!("{} octave{}",octaves.round(),if octaves.round()==1. {""}else{"s"})} else {format!("{}:1",format!("{:.4}",2.0_f64.powf(octaves)).trim_end_matches('0').trim_end_matches('.'))};
                    let field=ui.add_sized([48.,15.],egui::DragValue::new(&mut edit.draft.degrees[last]).speed(1.).range(1.0..=96000.0).suffix(" c")).on_hover_text(period_label);
                    #[cfg(test)] trace(ui,"edit-period",field.rect);
                    #[cfg(not(test))] let _=field;
                    ui.add_space(6.);
                    ui.label("Δ").on_hover_text("Show each value as a difference from the selected reference.");
                    let root=ui.selectable_value(&mut edit.mode,0,"Root").on_hover_text("Distance from the root, in cents.");
                    #[cfg(test)] trace(ui,"edit-delta-root",root.rect);
                    #[cfg(not(test))] let _=root;
                    ui.label("|");
                    let equal=ui.selectable_value(&mut edit.mode,1,"Equal").on_hover_text("Deviation from equal spacing across this period and note count, in cents.");
                    #[cfg(test)] trace(ui,"edit-delta-equal",equal.rect);
                    #[cfg(not(test))] let _=equal;
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                        ui.add_space(GRID_RIGHT_PADDING);
                        if let Ok(scale)=edit.draft.validated() {draw_scale_sized(ui,&ScaleShape::from_prepared(scale.tuning()),p,24.);}
                    });
                });
                ui.add_space(5.);
                let mut remove=None;
                let mut insert=None;
                let first=edit.page*notes_per_page;
                let end=((edit.page+1)*notes_per_page).min(edit.draft.degrees.len());
                let page_count=end-first;
                ui.scope(|ui| {
                ui.style_mut().text_styles.insert(egui::TextStyle::Body,FontId::proportional(TEXT_SMALL));
                ui.style_mut().text_styles.insert(egui::TextStyle::Button,FontId::proportional(TEXT_SMALL));
                ui.spacing_mut().interact_size.y=15.;
                ui.spacing_mut().button_padding.x=3.;
                let row_spacing=if note_count>20 {3.}else{1.};
                ui.horizontal(|ui| {
                ui.add_space(grid_left_margin);
                let note_grid=egui::Grid::new("edit-degrees").num_columns(column_count).spacing(vec2(column_gap,row_spacing)).show(ui,|ui| {
                    for row in 0..rows_per_column {
                        for column in 0..column_count {
                            let short=page_count/column_count;
                            let extra=page_count%column_count;
                            let column_len=short+usize::from(column<extra);
                            let column_start=first+column*short+column.min(extra);
                            let degree=column_start+row;
                            ui.allocate_ui_with_layout(vec2(NOTE_GROUP_WIDTH,15.),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                                ui.set_min_width(NOTE_GROUP_WIDTH);
                                ui.spacing_mut().item_spacing.x=NOTE_ITEM_GAP;
                                ui.spacing_mut().interact_size.x=0.;
                                if row>=column_len {return;}
                                ui.push_id(degree,|ui| {
                                    if degree==0 {
                                        ui.add_sized([NOTE_INDEX_WIDTH,15.],egui::Label::new("1"));
                                        ui.add_sized([NOTE_VALUE_WIDTH,15.],egui::Label::new("Root · 0"));
                                    }
                                    else {
                                        let n=degree-1;
                                        ui.add_sized([NOTE_INDEX_WIDTH,15.],egui::Label::new((degree+1).to_string()));
                                        let baseline=if edit.mode==1 {edit.draft.offset_baseline(n)}else{0.};
                                        let mut value=edit.draft.degrees[n]-baseline;
                                        let response=ui.add_sized([NOTE_VALUE_WIDTH,15.],egui::DragValue::new(&mut value).speed(0.1));
                                        if response.changed() {edit.draft.degrees[n]=value+baseline;}
                                        #[cfg(test)] trace(ui,&format!("edit-degree:{n}"),response.rect);
                                        let delete=ui.add_sized([NOTE_BUTTON_WIDTH,15.],egui::Button::new("×").small()).on_hover_text("Remove this note");
                                        #[cfg(test)] trace(ui,&format!("edit-remove-note:{n}"),delete.rect);
                                        if delete.clicked() {remove=Some(n);}
                                    }
                                    let add=ui.add_enabled(edit.draft.degrees.len()<4096,egui::Button::new("+").min_size(vec2(NOTE_BUTTON_WIDTH,15.))).on_hover_text("Insert a note after this one, halfway to the next pitch");
                                    #[cfg(test)] trace(ui,&format!("edit-insert-after:{degree}"),add.rect);
                                    if add.clicked() {insert=Some(degree);}
                                });
                            });
                        }
                        ui.end_row();
                    }
                });
                #[cfg(test)] trace(ui,"edit-note-grid",note_grid.response.rect);
                #[cfg(not(test))] let _=note_grid;
                });
                });
                if let Some(n)=remove {edit.error=edit.draft.remove_note(n).err();}
                if let Some(n)=insert {edit.error=edit.draft.insert_note(n).err();}
                if edit.draft.degrees.len()!=note_count {edit.count=edit.draft.degrees.len();}
                ui.add_space(5.);
                let keyboard = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x=0.;
                    ui.add_space(grid_left_margin);
                    for column in 0..column_count {
                        ui.allocate_ui_with_layout(vec2(NOTE_GROUP_WIDTH,16.),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                        ui.set_min_width(NOTE_GROUP_WIDTH);
                        ui.allocate_ui_with_layout(vec2(NOTE_GROUP_WIDTH-INSERT_TAIL_WIDTH,16.),egui::Layout::right_to_left(egui::Align::Center),|ui| {
                        ui.set_min_width(NOTE_GROUP_WIDTH-INSERT_TAIL_WIDTH);
                        ui.spacing_mut().item_spacing.x=NOTE_ITEM_GAP;
                        match column {
                        0 => {
                            let field=ui.add_sized([32.,15.],egui::DragValue::new(&mut edit.draft.root).range(0..=127));
                            #[cfg(test)] trace(ui,"edit-root-key",field.rect);
                            #[cfg(not(test))] let _=field;
                            ui.label("Root key");
                        }
                        1 => {
                            let field=ui.add_sized([32.,15.],egui::DragValue::new(&mut edit.draft.reference).range(-256..=255));
                            #[cfg(test)] trace(ui,"edit-reference-key",field.rect);
                            #[cfg(not(test))] let _=field;
                            ui.label("Reference");
                        }
                        _ => {
                        let replace=ui.small_button("Equal scale…");
                        #[cfg(test)] trace(ui,"edit-replace-equal",replace.rect);
                        egui::Popup::menu(&replace).show(|ui| {
                            ui.set_width(230.);
                            ui.label("Replaces every interval and resets keyboard mapping.");
                            ui.horizontal(|ui| {
                                ui.label("Notes");
                                ui.add(egui::DragValue::new(&mut edit.count).range(1..=4096));
                                if ui.button("Replace").clicked() {
                                    edit.error=edit.draft.equal_divisions(edit.count,*edit.draft.degrees.last().unwrap()).err();
                                    edit.page=0;
                                    ui.close();
                                }
                            });
                        });
                        if edit.draft.has_custom_mapping() {
                            let reset=small_icon(ui,"edit-default-mapping",Icon::Refresh,"Use default mapping · discard the imported keyboard mapping",p);
                            if reset.clicked() {edit.draft.reset_mapping();}
                        }
                        }
                        }
                        });
                        });
                        if column+1<column_count {ui.add_space(column_gap);}
                    }
                });
                #[cfg(test)] trace(ui,"edit-keyboard-mapping",keyboard.response.rect);
                #[cfg(not(test))] let _ = keyboard;
            if let Some(error)=&edit.error {ui.label(RichText::new(error).small().color(p.orange));}

            ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                ui.add_space(6.);
                ui.allocate_ui_with_layout(vec2(ui.available_width(),18.),egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    ui.add_space(6.);
                    let response = ui.button("Apply");
                    #[cfg(test)] trace(ui,"edit-apply",response.rect);
                    apply = response.clicked();
                    let response = ui.small_button("Cancel");
                    #[cfg(test)] trace(ui,"edit-cancel",response.rect);
                    cancel |= response.clicked();
                    let response=small_icon(ui,"edit-audition",Icon::Speaker(edit.audition),"Audition draft · temporarily hear changes without applying",p);
                    #[cfg(test)] trace(ui,"edit-audition",response.rect);
                    if response.clicked() {
                        edit.audition = !edit.audition;
                        if edit.audition {self.shared.audition_enabled.store(true,SeqCst);} else {self.shared.stop_audition();}
                    }
                });
            });

        });
        if let Some(redo) = history_action {
            let current = edit.draft.state();
            if let Some(restored) = edit.history.step(current, redo) {
                edit.draft.restore_state(restored);
                edit.count = edit.draft.degrees.len();
                edit.page = edit
                    .page
                    .min(edit.draft.degrees.len().div_ceil(24).saturating_sub(1));
                edit.error = None;
            }
        } else {
            let after = edit.draft.state();
            edit.history
                .observe(before, &after, ui.input(|input| input.pointer.any_down()));
        }
        if edit.audition {
            match edit.draft.validated().map(|scale| {
                self.shared.audition_enabled.store(true, SeqCst);
                self.shared.audition_prepared(scale);
            }) {
                Ok(()) => {}
                Err(error) => {
                    edit.error = Some(error);
                    self.shared.stop_audition();
                    edit.audition = false;
                }
            }
        }
        if cancel {
            self.shared.stop_audition();
            self.edit = None;
        } else if apply {
            let result = edit.draft.validated().and_then(|scale| {
                if edit.append {
                    self.shared.append_prepared(scale)
                } else {
                    self.shared
                        .assign_prepared(edit.destination, scale)
                        .map(|()| edit.destination)
                }
            });
            match result {
                Ok(slot) => {
                    self.edit = None;
                    self.inspect_slot(slot);
                    self.host.request_callback();
                }
                Err(error) => edit.error = Some(error),
            }
        }
    }
}

pub(super) struct ScaleEdit {
    pub(super) draft: inton_core::scale_edit::Draft,
    pub(super) destination: usize,
    pub(super) append: bool,
    pub(super) mode: usize,
    pub(super) page: usize,
    pub(super) count: usize,
    pub(super) audition: bool,
    pub(super) error: Option<String>,
    pub(super) history: DraftHistory,
}
#[derive(Default)]
pub(super) struct DraftHistory {
    pub(super) undo: Vec<inton_core::scale_edit::DraftState>,
    pub(super) redo: Vec<inton_core::scale_edit::DraftState>,
    pub(super) pending: Option<inton_core::scale_edit::DraftState>,
    pub(super) last_change: Option<Instant>,
}
impl DraftHistory {
    pub(super) fn can_undo(&self) -> bool {
        self.pending.is_some() || !self.undo.is_empty()
    }
    pub(super) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub(super) fn finish_pending(&mut self) {
        let Some(before) = self.pending.take() else {
            return;
        };
        if self.undo.len() == 32 {
            self.undo.remove(0);
        }
        self.undo.push(before);
        self.last_change = None;
    }
    pub(super) fn observe(
        &mut self,
        before: inton_core::scale_edit::DraftState,
        after: &inton_core::scale_edit::DraftState,
        pointer_down: bool,
    ) {
        if &before != after {
            if self.pending.is_none() {
                self.pending = Some(before);
            }
            self.redo.clear();
            self.last_change = Some(Instant::now());
        } else if !pointer_down
            && self
                .last_change
                .is_some_and(|changed| changed.elapsed() >= Duration::from_millis(400))
        {
            self.finish_pending();
        }
    }
    pub(super) fn step(
        &mut self,
        current: inton_core::scale_edit::DraftState,
        redo: bool,
    ) -> Option<inton_core::scale_edit::DraftState> {
        self.finish_pending();
        let source = if redo { &mut self.redo } else { &mut self.undo };
        let restored = source.pop()?;
        let destination = if redo { &mut self.undo } else { &mut self.redo };
        if destination.len() == 32 {
            destination.remove(0);
        }
        destination.push(current);
        Some(restored)
    }
}
