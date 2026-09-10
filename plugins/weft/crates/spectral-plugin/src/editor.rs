use crate::display_data::{ANALYZER_POINTS, AnalysisDisplay};
use crate::parameters::SpectralParams;
use crate::state::CurveState;
use controls::{
    CONTROL_GAP, estimated_note_transition_ms, fixed_envelope_parameter,
    fixed_motion_phase_size_parameter, fixed_motion_rate_parameter, fixed_parameter,
    fixed_step_parameter, global_footer, header,
};
use copypasta::ClipboardProvider;
use curve::{
    CurveAction, CurveTransformDrag, EditPoint, apply_curve_transform, cancel_curve_transform_drag,
    capture_spectrum_to_curve, curve_editor, curve_history_shortcut, curve_transform_is_neutral,
    finish_curve_transform_drag, flip_curve, pitch_ruler, set_curve_transform_defaults,
};
use egui::{Align2, FontId, Id, Key, LayerId, Order, Pos2, Rect, UiBuilder, Vec2};
use history::{CurveHistory, CurveSnapshot, HistoryEntry, restore_curve, restore_curve_snapshot};
use nice_plug::context::gui::GuiContext;
use nice_plug::params::Param;
use nice_plug_egui::NiceEguiApp;
use nice_plug_egui::baseview::HandlerError;
use oiko_ui::theme::{Palette, apply_theme};
use parameter_history::{ParameterGestures, TrackedParamSetter};
use rendering::{LineTrail, PlotContext, VisualMotionPhase, draw_harmonic_preview};
use spectral_dsp::{MANUAL_CURVE_MUTE_DB, MANUAL_MASK_POINTS, MotionConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
mod controls;
mod curve;
mod history;
mod rendering;

mod parameter_history;

pub(crate) const EDITOR_WIDTH: f32 = 760.0;
pub(crate) const EDITOR_HEIGHT: f32 = 560.0;
pub(crate) use oiko_ui::scale::closest_ui_scale;

pub struct SpectralEditor {
    tuning_cache: (crate::mts_client::Tuning, String),
    #[cfg(test)]
    test_tuning: Option<crate::mts_client::Tuning>,
    clipboard: Option<copypasta::ClipboardContext>,
    transfer_notice: Option<(String, Instant)>,
    params: Arc<SpectralParams>,
    display: Arc<AnalysisDisplay>,
    gui_context: Option<GuiContext>,
    last_curve_point: Option<EditPoint>,
    base_curve_snapshot: [f32; MANUAL_MASK_POINTS],
    smoothed_spectrum_db: [f32; ANALYZER_POINTS],
    capture_power_sum: [f64; ANALYZER_POINTS],
    capture_frames: u64,
    capture_active: bool,
    capture_epoch: u32,
    capture_seconds: f32,
    capture_preview: CurveState,
    output_peak: f32,
    transform_mode: bool,
    transform_drag: Option<CurveTransformDrag>,
    suppress_alt_transform: bool,
    curve_history: CurveHistory,
    parameter_gestures: ParameterGestures,
    displayed_note_depth_db: f32,
    displayed_motion_depth_db: f32,
    dark: Arc<AtomicBool>,
    about_open: bool,
    spectrum_range_menu_open: bool,
    last_spectrum_range_db: f32,
    pinned_note_drag_value: Option<bool>,
    particle_frame: crate::display_data::ParticleMask,
    orange_motion_trail: LineTrail,
    blue_motion_trail: LineTrail,
    visual_motion_phase: VisualMotionPhase,
    ruler_intro_started: Instant,
}

impl SpectralEditor {
    pub(crate) fn new(params: Arc<SpectralParams>, display: Arc<AnalysisDisplay>) -> Self {
        let displayed_note_depth_db = params.note_depth_db.value();
        let displayed_motion_depth_db = params.motion_depth_db.value();
        let last_spectrum_range_db = params.spectrum_range.get();
        Self {
            clipboard: None,
            tuning_cache: (crate::mts_client::Tuning::default(), String::new()),
            #[cfg(test)]
            test_tuning: None,
            transfer_notice: None,
            params,
            display,
            gui_context: None,
            last_curve_point: None,
            base_curve_snapshot: [0.0; MANUAL_MASK_POINTS],
            smoothed_spectrum_db: [MANUAL_CURVE_MUTE_DB; ANALYZER_POINTS],
            capture_power_sum: [0.0; ANALYZER_POINTS],
            capture_frames: 0,
            capture_active: false,
            capture_epoch: 0,
            capture_seconds: 0.0,
            capture_preview: CurveState::default(),
            output_peak: 0.0,
            transform_mode: false,
            transform_drag: None,
            suppress_alt_transform: false,
            curve_history: CurveHistory::default(),
            parameter_gestures: ParameterGestures::default(),
            displayed_note_depth_db,
            displayed_motion_depth_db,
            dark: Arc::new(AtomicBool::new(true)),
            about_open: false,
            spectrum_range_menu_open: false,
            last_spectrum_range_db,
            pinned_note_drag_value: None,
            particle_frame: crate::display_data::ParticleMask::default(),
            orange_motion_trail: LineTrail::default(),
            blue_motion_trail: LineTrail::default(),
            visual_motion_phase: VisualMotionPhase::default(),
            ruler_intro_started: Instant::now(),
        }
    }
}

impl NiceEguiApp for SpectralEditor {
    fn build(
        &mut self,
        context: egui::Context,
        gui_context: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), HandlerError> {
        self.gui_context = Some(gui_context);
        self.ruler_intro_started = Instant::now();
        self.initialize_view(&context);
        Ok(())
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, frame: &mut nice_plug_egui::Frame) {
        frame.set_key_capture(host_key_capture(
            self.curve_history.is_active(),
            root_ui.ctx().text_edit_focused(),
        ));
        self.draw_ui(root_ui);
        if let Some(scale) = oiko_ui::resize_grip::show(
            root_ui,
            Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT),
            self.params.ui_scale.get(),
        ) {
            self.params.ui_scale.set(scale);
            request_settled_scale(root_ui.ctx(), scale);
        }
    }

    fn editor_closed(&mut self) {
        if let Some(context) = &self.gui_context {
            self.parameter_gestures
                .finish(&self.params, &context.param_setter());
            let changes = self.parameter_gestures.take_completed();
            if !changes.is_empty() {
                self.curve_history
                    .record_before(HistoryEntry::Parameters(changes));
            }
        }
        self.gui_context = None;
        self.capture_active = false;
        self.display.capture.stop();
    }
}

impl Drop for SpectralEditor {
    fn drop(&mut self) {
        if let Some(context) = &self.gui_context {
            self.parameter_gestures
                .finish(&self.params, &context.param_setter());
        }
        if self.capture_active {
            self.display.capture.stop();
        }
    }
}

impl SpectralEditor {
    fn initialize_view(&self, context: &egui::Context) {
        apply_theme(context, self.dark.load(Ordering::Relaxed));
        let settled_scale = closest_ui_scale(self.params.ui_scale.get());
        self.params.ui_scale.set(settled_scale);
        // The host can create the editor before restoring the saved zoom. On
        // every window open, reconcile its geometry with the current setting
        // through the backend's deferred resize path.
        oiko_ui::scale::initialize_scale(
            context,
            settled_scale,
            Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT),
        );
    }

    fn draw_ui(&mut self, root_ui: &mut egui::Ui) {
        root_ui
            .ctx()
            .request_repaint_after(Duration::from_millis(16));
        let palette = Palette::new(self.dark.load(Ordering::Relaxed));
        root_ui
            .painter()
            .rect_filled(root_ui.max_rect(), 0.0, palette.panel);

        let render_scale = oiko_ui::scale::canvas_scale(self.params.ui_scale.get());
        let transform = egui::emath::TSTransform::from_scaling(render_scale);
        let content_layer = LayerId::new(Order::Middle, Id::new("weft-scaled-content"));
        let range_layer = LayerId::new(Order::Foreground, Id::new("spectrum-range-area"));
        root_ui.ctx().set_transform_layer(content_layer, transform);
        root_ui.ctx().set_transform_layer(range_layer, transform);

        let content_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT));
        let mut content_ui = root_ui.new_child(
            UiBuilder::new()
                .id_salt("weft-scaled-content")
                .layer_id(content_layer)
                .max_rect(content_rect)
                .layout(*root_ui.layout()),
        );
        content_ui.set_clip_rect(content_rect);
        {
            let ui = &mut content_ui;
            ui.set_min_size(Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT));
            ui.painter().rect_filled(ui.max_rect(), 0.0, palette.panel);

            header(ui, &self.dark, &mut self.about_open, &self.params.ui_scale);

            let precise = self.params.quality.value() == crate::parameters::FftQuality::Precise;
            for (index, smoothed) in self.smoothed_spectrum_db.iter_mut().enumerate() {
                let target = self.display.spectrum_level(index);
                let coefficient = if precise {
                    if target > *smoothed { 0.18 } else { 0.04 }
                } else if target > *smoothed {
                    0.45
                } else {
                    0.08
                };
                *smoothed += (target - *smoothed) * coefficient;
            }
            let sample_rate = self.display.sample_rate();
            let fft_size = self.params.quality.value().size();
            self.params.curve.copy_to(&mut self.base_curve_snapshot);
            if self.capture_active
                && let Some((power, seconds)) = self.display.capture.read(self.capture_epoch)
            {
                self.capture_power_sum = power;
                self.capture_seconds = seconds;
                self.capture_frames = u64::from(capture_spectrum_to_curve(
                    &power,
                    1,
                    &self.capture_preview,
                    sample_rate,
                    fft_size,
                ));
            }
            let elapsed = ui.input(|input| input.stable_dt).min(0.25);
            self.output_peak = self
                .display
                .take_output_peak()
                .max(self.output_peak * (-elapsed * 3.0).exp());
            let setter = self
                .gui_context
                .as_ref()
                .expect("GUI context is available while the editor is open")
                .param_setter();
            if self.suppress_alt_transform && !ui.input(|input| input.pointer.any_down()) {
                self.suppress_alt_transform = false;
            }
            let about_closed_by_escape = self.about_open
                && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape));
            if about_closed_by_escape {
                self.about_open = false;
            } else if consume_escape_for_temporary_mode(
                ui.ctx(),
                self.capture_active || self.transform_mode || self.transform_drag.is_some(),
            ) {
                self.capture_active = false;
                self.display.capture.stop();
                self.capture_power_sum.fill(0.0);
                self.capture_frames = 0;
                self.transform_mode = false;
                if let Some(active_drag) = self.transform_drag.take() {
                    self.suppress_alt_transform = true;
                    cancel_curve_transform_drag(active_drag, &self.params, &setter);
                }
            }
            if self.transform_drag.is_none() && !curve_transform_is_neutral(&self.params) {
                apply_curve_transform(
                    &self.base_curve_snapshot,
                    &self.params.curve,
                    sample_rate,
                    self.params.curve_depth_percent.value(),
                    self.params.curve_tilt_db_per_octave.value(),
                    self.params.curve_shift_semitones.value(),
                );
                set_curve_transform_defaults(&self.params, &setter);
                self.params.curve.copy_to(&mut self.base_curve_snapshot);
            }
            let note_depth_target = self.params.note_depth_db.value();
            self.displayed_note_depth_db +=
                (note_depth_target - self.displayed_note_depth_db) * 0.14;
            let motion_depth_target = self.params.motion_depth_db.value();
            self.displayed_motion_depth_db +=
                (motion_depth_target - self.displayed_motion_depth_db) * 0.14;
            let display_range_db = self.params.spectrum_range.get();
            if (display_range_db - self.last_spectrum_range_db).abs() > 0.1 {
                self.orange_motion_trail = LineTrail::default();
                self.blue_motion_trail = LineTrail::default();
                self.last_spectrum_range_db = display_range_db;
            }
            let visual_motion_phase = self.visual_motion_phase.update(self.display.motion_phase());
            let history_available =
                !self.parameter_gestures.is_active() && !ui.input(|input| input.pointer.any_down());
            let shortcut_action =
                curve_history_shortcut(ui.ctx(), &self.curve_history, history_available);
            let history_snapshot = CurveSnapshot::capture(&self.base_curve_snapshot, &self.params);
            let curve_output = curve_editor(
                ui,
                &self.params.curve,
                PlotContext {
                    display: &self.display,
                    spectrum_db: &self.smoothed_spectrum_db,
                    base_curve: &self.base_curve_snapshot,
                    sample_rate,
                    fft_size,
                    note_depth_db: self.displayed_note_depth_db,
                    note_control_mix: self
                        .params
                        .note_depth_db
                        .preview_normalized(self.displayed_note_depth_db),
                    note_width_cents: self.params.width_cents.value(),
                    partials: self.params.partials.value() as usize,
                    partial_rolloff_db: self.params.harmonic_rolloff_db.value(),
                    display_range_db,
                    motion: MotionConfig {
                        shape: self.params.motion_shape.value().into(),
                        depth_db: self.displayed_motion_depth_db,
                        phase: visual_motion_phase,
                        size_octaves: self.params.motion_size_octaves.value(),
                    },
                    particles: {
                        self.display.read_particles(&mut self.particle_frame);
                        &self.particle_frame
                    },
                    curve_depth_percent: self.params.curve_depth_percent.value(),
                    curve_tilt_db_per_octave: self.params.curve_tilt_db_per_octave.value(),
                    curve_shift_semitones: self.params.curve_shift_semitones.value(),
                    palette,
                },
                &mut self.last_curve_point,
                &mut self.orange_motion_trail,
                &mut self.blue_motion_trail,
                &self.params.spectrum_range,
                &mut self.spectrum_range_menu_open,
                self.capture_active,
                (self.capture_active && self.capture_frames > 0).then_some(&self.capture_preview),
                self.capture_seconds,
                &mut self.transform_mode,
                &mut self.transform_drag,
                self.suppress_alt_transform,
                self.about_open,
                self.curve_history.can_undo() && !self.parameter_gestures.is_active(),
                self.curve_history.can_redo() && !self.parameter_gestures.is_active(),
                &self.params,
                &setter,
            );
            match shortcut_action.or(curve_output.action) {
                Some(CurveAction::Reset) => {
                    self.capture_active = false;
                    self.display.capture.stop();
                    self.capture_power_sum.fill(0.0);
                    self.capture_frames = 0;
                    self.curve_history.record_before(history_snapshot);
                    self.params.curve.reset();
                    set_curve_transform_defaults(&self.params, &setter);
                }
                Some(CurveAction::ToggleCapture) if self.capture_active => {
                    self.capture_active = false;
                    self.display.capture.stop();
                    if capture_spectrum_to_curve(
                        &self.capture_power_sum,
                        self.capture_frames,
                        &self.params.curve,
                        sample_rate,
                        fft_size,
                    ) {
                        self.curve_history.record_before(history_snapshot);
                    }
                }
                Some(CurveAction::ToggleCapture) => {
                    self.capture_power_sum.fill(0.0);
                    self.capture_frames = 0;
                    self.capture_active = true;
                    self.capture_seconds = 0.0;
                    self.capture_preview.reset();
                    self.capture_epoch = self.display.capture.begin();
                    self.transform_mode = false;
                }
                Some(CurveAction::Flip) => {
                    if flip_curve(&self.base_curve_snapshot, &self.params.curve) {
                        self.curve_history.record_before(history_snapshot);
                    }
                }
                Some(action @ (CurveAction::Copy | CurveAction::Paste)) => {
                    let result = (|| -> Result<&str, String> {
                        if self.clipboard.is_none() {
                            self.clipboard = Some(
                                copypasta::ClipboardContext::new()
                                    .map_err(|_| "Clipboard unavailable".to_owned())?,
                            );
                        }
                        let clipboard = self.clipboard.as_mut().ok_or("Clipboard unavailable")?;
                        if matches!(action, CurveAction::Copy) {
                            clipboard
                                .set_contents(crate::curve_transfer::encode(
                                    &self.base_curve_snapshot,
                                    sample_rate,
                                ))
                                .map_err(|_| "Could not copy curve".to_owned())?;
                            Ok("Curve copied")
                        } else {
                            let text = clipboard
                                .get_contents()
                                .map_err(|_| "Could not read clipboard".to_owned())?;
                            let values = crate::curve_transfer::decode(&text, sample_rate)
                                .map_err(str::to_owned)?;
                            self.curve_history.record_before(history_snapshot);
                            restore_curve(&self.params.curve, &values);
                            Ok("Curve pasted")
                        }
                    })();
                    let notice = match result {
                        Ok(text) => text.to_owned(),
                        Err(error) => error,
                    };
                    self.transfer_notice = Some((notice, Instant::now()));
                }
                Some(action @ (CurveAction::Save | CurveAction::Load)) => {
                    let dialog = rfd::FileDialog::new().add_filter("Weft curve", &["weftcurve"]);
                    if matches!(action, CurveAction::Save) {
                        if let Some(path) = dialog.set_file_name("Untitled.weftcurve").save_file() {
                            let text = crate::curve_transfer::encode(
                                &self.base_curve_snapshot,
                                sample_rate,
                            );
                            let notice = if std::fs::write(path, text).is_ok() {
                                "Curve saved"
                            } else {
                                "Could not save curve"
                            };
                            self.transfer_notice = Some((notice.to_owned(), Instant::now()));
                        }
                    } else if let Some(path) = dialog.pick_file() {
                        let result = (|| -> Result<(), String> {
                            use std::io::Read;
                            let file = std::fs::File::open(path)
                                .map_err(|_| "Could not open curve file")?;
                            let mut text = String::new();
                            file.take(1_000_001)
                                .read_to_string(&mut text)
                                .map_err(|_| "Could not read curve file")?;
                            let values = crate::curve_transfer::decode(&text, sample_rate)
                                .map_err(str::to_owned)?;
                            self.curve_history.record_before(history_snapshot);
                            restore_curve(&self.params.curve, &values);
                            Ok(())
                        })();
                        self.transfer_notice = Some((
                            result.err().unwrap_or_else(|| "Curve loaded".to_owned()),
                            Instant::now(),
                        ));
                    }
                }
                Some(CurveAction::BeginEdit) => self.curve_history.record_before(history_snapshot),
                Some(CurveAction::CommitTransform(active_drag)) => {
                    let mut before = history_snapshot;
                    before.depth_percent = 100.0;
                    before.tilt_db_per_octave = 0.0;
                    before.shift_semitones = 0.0;
                    self.curve_history.record_before(before);
                    apply_curve_transform(
                        &self.base_curve_snapshot,
                        &self.params.curve,
                        sample_rate,
                        self.params.curve_depth_percent.value(),
                        self.params.curve_tilt_db_per_octave.value(),
                        self.params.curve_shift_semitones.value(),
                    );
                    finish_curve_transform_drag(active_drag, &self.params, &setter);
                }
                Some(CurveAction::Undo) => {
                    if let Some(snapshot) = self.curve_history.undo(history_snapshot, &self.params)
                    {
                        restore_curve_snapshot(snapshot, &self.params.curve, &self.params, &setter);
                    }
                }
                Some(CurveAction::Redo) => {
                    if let Some(snapshot) = self.curve_history.redo(history_snapshot, &self.params)
                    {
                        restore_curve_snapshot(snapshot, &self.params.curve, &self.params, &setter);
                    }
                }
                None => {}
            }
            if let Some((notice, started)) = &self.transfer_notice
                && started.elapsed().as_secs_f32() < 3.0
            {
                ui.painter().text(
                    Pos2::new(
                        curve_output.rect.center().x,
                        curve_output.rect.bottom() - 70.0,
                    ),
                    Align2::CENTER_CENTER,
                    notice,
                    FontId::proportional(oiko_ui::typography::TEXT_SMALL),
                    palette.ink,
                );
            }
            if let Some(snapshot) = self.display.tuning_snapshot.read() {
                self.tuning_cache = snapshot;
            }
            let tuning = self.tuning_cache.0;
            #[cfg(test)]
            let tuning = self.test_tuning.unwrap_or(tuning);
            let tuning_name = if tuning.active {
                self.tuning_cache.1.as_str()
            } else {
                ""
            };
            let before_notes = HistoryEntry::Notes(Box::new(self.params.pinned_notes.snapshot()));
            let (notes_edit, hovered_note, _) = pitch_ruler(
                ui,
                &self.display,
                &self.params.pinned_notes,
                palette,
                &mut self.pinned_note_drag_value,
                self.ruler_intro_started.elapsed().as_secs_f32(),
                &tuning,
                tuning_name,
            );
            if notes_edit {
                self.curve_history.record_before(before_notes);
            }
            if !self.about_open
                && let Some(note) = hovered_note
            {
                draw_harmonic_preview(
                    &ui.painter_at(curve_output.rect),
                    curve_output.rect.shrink2(Vec2::new(10.0, 14.0)),
                    tuning.frequencies[note],
                    self.params.partials.value() as usize,
                    self.params.harmonic_rolloff_db.value(),
                    sample_rate,
                    palette,
                );
            }

            // Only ordinary GUI controls use the recorder. Curve transforms,
            // their neutral resets and history replay keep the plain setter.
            let setter = TrackedParamSetter::new(&setter, &self.params, &self.parameter_gestures);
            egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                let control_gap = CONTROL_GAP;
                let control_width = (ui.available_width() - control_gap * 4.0) / 5.0;
                ui.label(
                    egui::RichText::new("SPECTRAL MOTION")
                        .small()
                        .strong()
                        .color(palette.orange),
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = control_gap;
                    fixed_parameter(
                        ui,
                        control_width,
                        (true, palette.orange),
                        "DEPTH",
                        &self.params.motion_depth_db,
                        &setter,
                        "Maximum attenuation beneath the drawn orange ceiling. Zero is completely neutral.",
                    );
                    fixed_step_parameter(
                        ui,
                        control_width,
                        true,
                        "SHAPE",
                        &self.params.motion_shape,
                        &setter,
                        "Ripple, harmonic comb, drift, scan, notch, softened saw, plucked Sprinkle gestures or swelling Cloud envelopes. Both run freely without notes and follow live or pinned pitches when present. Low particles are quieter; draw on the spectrum to control the bass.",
                    );
                    fixed_motion_rate_parameter(
                        ui,
                        control_width,
                        (true, palette.orange),
                        &self.params.motion_rate_hz,
                        &self.params.motion_sync,
                        &self.params.motion_rate_division,
                        self.display.tempo(),
                        self.display.sample_rate(),
                        self.params.quality.value().size(),
                        &setter,
                        "Motion speed. Sprinkle shares this pace across interwoven groups and rests; faster rates increase activity and overlap.",
                    );
                    fixed_step_parameter(
                        ui,
                        control_width,
                        true,
                        "DIRECTION",
                        &self.params.motion_direction,
                        &setter,
                        "Forward, reverse, or alternating motion for looping shapes. Sprinkle biases octave choices. Cloud selects rounded, reverse-swell, or alternating envelopes; centers stay in place.",
                    );
                    fixed_motion_phase_size_parameter(
                        ui,
                        control_width,
                        (true, palette.orange),
                        &self.params.motion_phase_percent,
                        &self.params.motion_size_octaves,
                        &setter,
                    );
                });

                let intrinsic_note_transition_ms = estimated_note_transition_ms(
                    self.params.quality.value().size(),
                    self.display.sample_rate(),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("NOTE CONTROL")
                        .small()
                        .strong()
                        .color(palette.blue),
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = control_gap;
                    fixed_parameter(
                        ui,
                        control_width,
                        (true, palette.blue),
                        "DEPTH",
                        &self.params.note_depth_db,
                        &setter,
                        "Increases contrast by lowering the background and gently emphasizing played spectral regions.",
                    );
                    fixed_parameter(
                        ui,
                        control_width,
                        (true, palette.blue),
                        "WIDTH",
                        &self.params.width_cents,
                        &setter,
                        "The spread around each input note. Wider skirts progressively need less automatic note emphasis.",
                    );
                    fixed_parameter(
                        ui,
                        control_width,
                        (true, palette.blue),
                        "PARTIALS",
                        &self.params.partials,
                        &setter,
                        "Number of harmonics in the note mask. Also sets Sprinkle’s available harmonics, with or without notes; each event chooses mostly one, occasionally two or three.",
                    );
                    fixed_parameter(
                        ui,
                        control_width,
                        (true, palette.blue),
                        "PARTIAL ROLLOFF",
                        &self.params.harmonic_rolloff_db,
                        &setter,
                        "How much higher note-mask partials fade per octave. Also makes higher Sprinkle harmonics less likely to be chosen.",
                    );
                    fixed_envelope_parameter(
                        ui,
                        control_width,
                        (true, palette.blue),
                        "NOTE FADE",
                        &self.params.note_attack_ms,
                        &self.params.note_release_ms,
                        intrinsic_note_transition_ms,
                        &setter,
                        "Attack and Release shape note regions and each new Sprinkle, including without MIDI or at zero Note Depth. Sprinkle decays after its peak independently of note-off. Display includes estimated STFT handover; very short sprinkles have a resolution-dependent minimum.",
                    );
                });
            });

            global_footer(ui, palette, &self.params, &setter, self.output_peak);
            // Automation can hide a dragged control (for example FREE/SYNC).
            // Its widget then cannot emit drag_stopped, so close the gesture
            // on pointer release even if that widget is no longer drawn.
            if self.parameter_gestures.is_active() && !ui.input(|input| input.pointer.any_down()) {
                self.parameter_gestures.finish(
                    &self.params,
                    &self.gui_context.as_ref().unwrap().param_setter(),
                );
            }
            let changes = self.parameter_gestures.take_completed();
            if !changes.is_empty() {
                self.curve_history
                    .record_before(HistoryEntry::Parameters(changes));
            }
        }
    }
}

fn host_key_capture(
    curve_shortcuts_active: bool,
    text_edit_focused: bool,
) -> nice_plug_egui::KeyCapture {
    if text_edit_focused {
        nice_plug_egui::KeyCapture::CaptureAll
    } else if curve_shortcuts_active {
        nice_plug_egui::KeyCapture::CaptureCommands(vec![
            nice_plug_egui::Key::Character("z".into()),
            nice_plug_egui::Key::Character("Z".into()),
        ])
    } else {
        nice_plug_egui::KeyCapture::IgnoreAll
    }
}

fn consume_escape_for_temporary_mode(context: &egui::Context, mode_active: bool) -> bool {
    mode_active && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape))
}

fn request_settled_scale(context: &egui::Context, scale: f32) {
    oiko_ui::scale::request_scale(context, scale, Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT));
}

#[cfg(test)]
#[path = "editor_test_ui.rs"]
mod test_ui;

#[cfg(test)]
mod tests;
