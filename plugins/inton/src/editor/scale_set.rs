//! Project slot selection, reordering and scale-set interactions.
use super::rendering::{Icon, icon, small_icon};
#[cfg(test)]
use super::trace;
use super::{Command, Editor, ScaleDrag, SlotDrag};
use crate::editor_model::{AssignmentIntent, Inspection};
use crate::editor_theme::{Palette, TEXT_SMALL};
use egui::{RichText, ScrollArea, Sense, Stroke, pos2, vec2};
use inton_core::engine::Parameter;
use inton_core::state::Project;
use std::time::Duration;

impl Editor {
    pub(super) fn inspect_slot(&mut self, n: usize) {
        self.assignment = AssignmentIntent::Browse;
        self.focus_library = false;
        self.set.destination = n;
        self.inspection = Inspection::Slot(n);
        self.preview = None;
        self.shared.stop_audition();
    }
    pub(super) fn activate_slot(&mut self, n: usize) {
        self.inspect_slot(n);
        self.inspection = Inspection::Active;
        self.set_parameter(Parameter::Position, n as f64);
    }
    pub(super) fn set_panel(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        project: &Project,
        active: usize,
        height: f32,
    ) {
        let follow_active = self.last_active != Some(active);
        if follow_active
            && self.inspection.slot().is_none()
            && self.last_active == Some(self.set.destination)
        {
            self.set.destination = active;
        }
        let follow_destination = self.last_destination != Some(self.set.destination);
        let rows = self.set.rows(project, active);
        let mut clear = None;
        let mut replace = None;
        let mut append = None;
        let mut movement = None;
        let scroll = ScrollArea::vertical().id_salt("project-set")
            .max_height((height-24.).max(46.)).auto_shrink([false,false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y=3.;
                if (egui::DragAndDrop::has_payload_of_type::<ScaleDrag>(ui.ctx()) || egui::DragAndDrop::has_payload_of_type::<SlotDrag>(ui.ctx()))
                    && let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let clip = ui.clip_rect();
                    if clip.contains(pos) {
                        if pos.y < clip.top() + 18. { ui.scroll_with_delta(vec2(0., 6.)); }
                        else if pos.y > clip.bottom() - 18. { ui.scroll_with_delta(vec2(0., -6.)); }
                    }
                }
                let mut row_width = ui.available_width();
                for n in rows {
                    ui.push_id(n, |ui| {
                        let flash = self.acknowledgment.as_ref().is_some_and(|(slot,t)| *slot==n && t.elapsed()<Duration::from_millis(700));
                        let row = egui::Frame::new().fill(if flash {p.track} else {p.panel})
                            .stroke(Stroke::new(1., if self.assignment != AssignmentIntent::Append && self.inspection.slot()==Some(n) {p.blue} else {p.rule}))
                            .inner_margin(1).corner_radius(3).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if icon(ui,"activate",Icon::SlotStatus(n==active),"Activate this slot",p).clicked() { self.activate_slot(n); }
                                    let label = project.label(n);
                                    let width = (ui.available_width()-64.).max(20.);
                                    let r = ui.add_sized([width,22.], egui::Button::new(RichText::new(&label).size(TEXT_SMALL).color(if n==active {p.orange} else {p.ink}))
                                        .frame(false).right_text("").truncate().sense(Sense::click_and_drag()))
                                        .on_hover_text(format!("{label}\nClick to inspect; double-click to activate; drag to reorder"));
                                    #[cfg(test)] trace(ui,&format!("slot:{n}"),r.rect);
                                    if r.clicked() { self.inspect_slot(n); }
                                    if r.double_clicked() { self.activate_slot(n); }
                                    if project.slots[n].is_some() { r.dnd_set_drag_payload(SlotDrag(n)); }
                                    let browse = icon(ui,"replace-slot",if project.slots[n].is_some() {Icon::Refresh}else{Icon::Plus},if project.slots[n].is_some() {"Choose a replacement for this slot"} else {"Choose a scale for this empty slot"},p);
                                    #[cfg(test)] trace(ui,&format!("replace-slot:{n}"),browse.rect);
                                    if browse.clicked() { self.begin_replace_scale(n); }
                                    let remove = icon(ui,"clear",Icon::Close,if project.slots[n+1..].iter().any(Option::is_some) {"Clear contents; keep this slot and its automation number"} else {"Remove this slot; no occupied slots follow it"},p);
                                    #[cfg(test)] trace(ui,&format!("clear:{n}"),remove.rect);
                                    if remove.clicked() { clear=Some(n); }
                                });
                            });
                        row_width = row.response.rect.width();
                        if (follow_active && n==active) || (!follow_active && follow_destination && n==self.set.destination) {
                            row.response.scroll_to_me(Some(egui::Align::Center));
                        }
                        let r = ui.interact(row.response.rect,ui.id().with("drop"),Sense::hover());
                        if r.dnd_hover_payload::<ScaleDrag>().is_some() {
                            ui.painter().rect_stroke(r.rect,3,Stroke::new(2.,p.orange),egui::StrokeKind::Inside);
                        }
                        if r.dnd_hover_payload::<ScaleDrag>().is_some()
                            && let Some(payload)=r.dnd_release_payload::<ScaleDrag>() { replace=Some((payload.0.clone(),n)); }
                        if let Some(payload)=r.dnd_hover_payload::<SlotDrag>() {
                            let before = if ui.input(|i| i.pointer.hover_pos()).is_some_and(|pos| pos.y < r.rect.center().y) {n} else {n+1};
                            let to = if payload.0 < before {before-1} else {before};
                            let y = if before==n {r.rect.top()-1.} else {r.rect.bottom()+1.};
                            ui.painter().line_segment([pos2(r.rect.left(),y),pos2(r.rect.right(),y)],Stroke::new(2.,p.orange));
                            if r.dnd_release_payload::<SlotDrag>().is_some() { movement=Some((payload.0,to)); }
                        }
                    });
                }
                if (0..32).all(|n| project.has_slot(n)) { return ui.cursor().top(); }
                // This is an add target, not a stored/numbered empty slot.
                let (rect,response)=ui.allocate_exact_size(vec2(row_width,24.),Sense::click());
                let response=response.on_hover_text("Add a scale · click to browse, or drop a scale here");
                #[cfg(test)] trace(ui,"add-scale",rect);
                let hovered=rect.contains(ui.input(|i|i.pointer.hover_pos()).unwrap_or(egui::Pos2::ZERO));
                let color=if self.assignment == AssignmentIntent::Append {p.blue} else if hovered {p.ink} else {p.muted};
                if self.assignment == AssignmentIntent::Append || hovered {ui.painter().rect_filled(rect,3,p.panel);}
                let outline=rect.shrink(0.5);
                for (a,b) in [(outline.left_top(),outline.right_top()),(outline.right_top(),outline.right_bottom()),(outline.right_bottom(),outline.left_bottom()),(outline.left_bottom(),outline.left_top())] {
                    let length=a.distance(b);let direction=(b-a)/length;
                    let mut offset=0.;
                    while offset<length {ui.painter().line_segment([a+direction*offset,a+direction*(offset+5.).min(length)],Stroke::new(1.,color));offset+=9.;}
                }
                let c=rect.center();
                ui.painter().line_segment([c-vec2(5.,0.),c+vec2(5.,0.)],Stroke::new(1.3,color));
                ui.painter().line_segment([c-vec2(0.,5.),c+vec2(0.,5.)],Stroke::new(1.3,color));
                if response.clicked() {self.begin_add_scale();}
                rect.top()
            });
        ui.horizontal(|ui| {
            let ready=self.job.is_none() && self.queue.is_empty() && self.edit.is_none();
            let response=ui.add_enabled_ui(ready && project.uses_scale_set(), |ui| small_icon(ui,"clear-scale-set",Icon::Trash,"Clear scale set",p)).inner;
            #[cfg(test)] trace(ui,"clear-scale-set",response.rect);
            let response=response.on_hover_ui(|ui| {ui.set_max_width(220.);ui.label("Keep the active tuning as a single scale. Slot assignments reset; Undo restores the set.");});
            if response.clicked() {self.set_history_action(0);}
        });
        let drop_rect = egui::Rect::from_min_max(
            pos2(
                scroll.inner_rect.left(),
                scroll.inner.min(scroll.inner_rect.bottom()),
            ),
            scroll.inner_rect.max,
        );
        let r = ui.interact(drop_rect, ui.id().with("append-scale"), Sense::hover());
        #[cfg(test)]
        trace(ui, "append-scale", drop_rect);
        if r.dnd_hover_payload::<ScaleDrag>().is_some() {
            ui.painter().rect_stroke(
                drop_rect,
                3,
                Stroke::new(2., p.orange),
                egui::StrokeKind::Inside,
            );
        }
        if r.dnd_hover_payload::<ScaleDrag>().is_some()
            && let Some(payload) = r.dnd_release_payload::<ScaleDrag>()
        {
            append = Some(payload.0.clone());
        }
        if let Some(payload) = r.dnd_hover_payload::<SlotDrag>() {
            ui.painter().line_segment(
                [drop_rect.left_top(), drop_rect.right_top()],
                Stroke::new(2., p.orange),
            );
            if r.dnd_release_payload::<SlotDrag>().is_some() {
                movement = Some((
                    payload.0,
                    project.slots.iter().rposition(Option::is_some).unwrap_or(0),
                ));
            }
        }
        self.last_active = Some(active);
        self.last_destination = Some(self.set.destination);
        if let Some(n) = clear {
            if let Err(e) = self.shared.clear(n) {
                self.message = Some(e);
            } else {
                self.set.revealed.remove(&n);
                let new_active = self.shared.snapshot().parameters.position;
                if n == active && new_active != active {
                    self.inspect_slot(new_active);
                    self.inspection = Inspection::Active;
                }
                if !self.shared.snapshot().has_slot(n) && n != active && self.set.destination == n {
                    self.set.destination = active;
                    self.inspection = Inspection::Active;
                    self.last_destination = None;
                }
            }
            self.host.request_callback();
        }
        if let Some((id, n)) = replace {
            self.assign_scale(id, n);
        }
        if let Some(id) = append {
            self.enqueue(Command::Append(id));
        }
        if let Some((from, to)) = movement {
            match self.shared.move_slot(from, to) {
                Ok(()) => {
                    self.set.destination =
                        inton_core::state::moved_index(self.set.destination, from, to);
                    if let Inspection::Slot(n) = &mut self.inspection {
                        *n = inton_core::state::moved_index(*n, from, to);
                    }
                    self.set.revealed = self
                        .set
                        .revealed
                        .iter()
                        .map(|n| inton_core::state::moved_index(*n, from, to))
                        .collect();
                    self.last_active = Some(inton_core::state::moved_index(active, from, to));
                    self.last_destination = None;
                    self.message = None;
                    self.host.request_callback();
                }
                Err(e) => self.message = Some(e),
            }
        }
    }
}
