//! Spectrum, curves, note overlays and plot coordinates.
use super::curve::{CurveTransformZone, SOFT_PENCIL_RADIUS_PX};
use crate::MAX_SPLASH_EVENTS;
use crate::curve::transformed_curve_db;
use crate::display_data::{ANALYZER_POINTS, AnalysisDisplay, display_max_frequency};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};
use oiko_dsp::{db_to_gain, note_frequency_with_tuning};
use oiko_ui::theme::Palette;
use spectral_dsp::MANUAL_CURVE_MUTE_DB;
use spectral_dsp::MANUAL_MASK_POINTS;
use spectral_dsp::MIDI_NOTES;
use spectral_dsp::MIN_DISPLAY_FREQUENCY_HZ;
use spectral_dsp::MotionConfig;
use spectral_dsp::MotionShape;
use spectral_dsp::SplashEvent;
use spectral_dsp::motion_attenuation_db;
use spectral_dsp::splash_attenuation_db;
use std::f32::consts::PI;
use std::time::{Duration, Instant};

const RULER_INTRO_DURATION_SECONDS: f32 = 0.5;
const MOTION_TRAIL_CAPTURE_INTERVAL: Duration = Duration::from_millis(42);
const MOTION_TRAIL_FRAMES: usize = 2;
const MOTION_OUTLINE_POINTS: usize = 512;
pub(super) struct LineTrail {
    pub(super) frames: Vec<Vec<Pos2>>,
    pub(super) last_phase: Option<f32>,
    pub(super) last_capture: Instant,
}

#[derive(Clone, Copy)]
pub(super) struct TrailStyle {
    pub(super) color: Color32,
    pub(super) width: f32,
    pub(super) visibility: f32,
    pub(super) active: bool,
}

pub(super) struct VisualMotionPhase {
    pub(super) last_target: f32,
    pub(super) last_target_time: Instant,
    pub(super) velocity: f32,
    pub(super) initialized: bool,
}

impl Default for VisualMotionPhase {
    fn default() -> Self {
        Self {
            last_target: 0.0,
            last_target_time: Instant::now(),
            velocity: 0.0,
            initialized: false,
        }
    }
}

impl VisualMotionPhase {
    pub(super) fn update(&mut self, target: f32) -> f32 {
        let now = Instant::now();
        if !self.initialized {
            self.last_target = target;
            self.last_target_time = now;
            self.initialized = true;
            return target;
        }

        let delta = (target - self.last_target + 0.5).rem_euclid(1.0) - 0.5;
        if delta.abs() > 1.0e-6 {
            let elapsed = now
                .duration_since(self.last_target_time)
                .as_secs_f32()
                .max(1.0 / 240.0);
            let measured_velocity = (delta / elapsed).clamp(-8.0, 8.0);
            self.velocity += (measured_velocity - self.velocity) * 0.72;
            self.last_target = target;
            self.last_target_time = now;
        }

        let age = now.duration_since(self.last_target_time).as_secs_f32();
        let extrapolation = age.min(0.10);
        let stale_fade = (1.0 - (age - 0.10).max(0.0) / 0.10).clamp(0.0, 1.0);
        if stale_fade <= 0.0 {
            self.velocity = 0.0;
        }
        (self.last_target + self.velocity * extrapolation * stale_fade).rem_euclid(1.0)
    }
}

impl Default for LineTrail {
    fn default() -> Self {
        Self {
            frames: Vec::with_capacity(MOTION_TRAIL_FRAMES),
            last_phase: None,
            last_capture: Instant::now(),
        }
    }
}

impl LineTrail {
    pub(super) fn paint_and_capture(
        &mut self,
        painter: &egui::Painter,
        current: &[Pos2],
        phase: f32,
        style: TrailStyle,
    ) {
        if !style.active {
            self.frames.clear();
            self.last_phase = None;
            return;
        }

        let visibility = style.visibility.clamp(0.0, 1.0);
        for (index, frame) in self.frames.iter().enumerate() {
            let age = self.frames.len() - index;
            let alpha = match age {
                1 => 58.0,
                _ => 22.0,
            } * visibility;
            painter.add(egui::Shape::line(
                frame.clone(),
                Stroke::new(style.width, with_alpha(style.color, alpha.round() as u8)),
            ));
        }

        let now = Instant::now();
        let phase_moved = self.last_phase.is_none_or(|previous| {
            let distance = (phase - previous).abs();
            distance.min(1.0 - distance) > 0.0005
        });
        if phase_moved
            && (self.last_phase.is_none()
                || now.duration_since(self.last_capture) >= MOTION_TRAIL_CAPTURE_INTERVAL)
        {
            if self.frames.len() == MOTION_TRAIL_FRAMES {
                self.frames.remove(0);
            }
            self.frames.push(current.to_vec());
            self.last_phase = Some(phase);
            self.last_capture = now;
        }
    }
}

pub(super) struct PlotContext<'a> {
    pub(super) display: &'a AnalysisDisplay,
    pub(super) spectrum_db: &'a [f32; ANALYZER_POINTS],
    pub(super) base_curve: &'a [f32; MANUAL_MASK_POINTS],
    pub(super) sample_rate: f32,
    pub(super) fft_size: usize,
    pub(super) note_depth_db: f32,
    pub(super) note_control_mix: f32,
    pub(super) note_width_cents: f32,
    pub(super) partials: usize,
    pub(super) partial_rolloff_db: f32,
    pub(super) display_range_db: f32,
    pub(super) motion: MotionConfig,
    pub(super) splashes: [SplashEvent; MAX_SPLASH_EVENTS],
    pub(super) curve_depth_percent: f32,
    pub(super) curve_tilt_db_per_octave: f32,
    pub(super) curve_shift_semitones: f32,
    pub(super) palette: Palette,
}

pub(super) fn draw_transform_edge_glow(
    painter: &egui::Painter,
    frame: Rect,
    zone: CurveTransformZone,
    palette: Palette,
) {
    const EDGE_ZONE: f32 = 20.0;
    const BANDS: usize = 8;
    let band = EDGE_ZONE / BANDS as f32;
    for index in 0..BANDS {
        let near = index as f32 * band;
        let far = (index + 1) as f32 * band;
        let rect = match zone {
            CurveTransformZone::Depth => Rect::from_min_max(
                Pos2::new(frame.left(), frame.bottom() - far),
                Pos2::new(frame.right(), frame.bottom() - near),
            ),
            CurveTransformZone::TiltLow => Rect::from_min_max(
                Pos2::new(frame.left() + near, frame.top()),
                Pos2::new(frame.left() + far, frame.bottom() - EDGE_ZONE),
            ),
            CurveTransformZone::TiltHigh => Rect::from_min_max(
                Pos2::new(frame.right() - far, frame.top()),
                Pos2::new(frame.right() - near, frame.bottom() - EDGE_ZONE),
            ),
            CurveTransformZone::Shift => return,
        };
        let alpha = (30.0 * (1.0 - index as f32 / BANDS as f32).powi(2)).round() as u8;
        painter.rect_filled(rect, 0.0, with_alpha(palette.orange, alpha));
    }
}

pub(super) fn draw_transform_handles(
    painter: &egui::Painter,
    frame: Rect,
    active_zone: Option<CurveTransformZone>,
    palette: Palette,
) {
    let handle_size = Vec2::new(8.0, 14.0);
    for (zone, center) in [
        (
            CurveTransformZone::TiltLow,
            Pos2::new(frame.left(), frame.center().y),
        ),
        (
            CurveTransformZone::TiltHigh,
            Pos2::new(frame.right(), frame.center().y),
        ),
        (
            CurveTransformZone::Depth,
            Pos2::new(frame.center().x, frame.bottom()),
        ),
    ] {
        let rect = Rect::from_center_size(center, handle_size);
        painter.rect_filled(
            rect,
            3.0,
            if active_zone == Some(zone) {
                palette.orange
            } else {
                palette.page
            },
        );
        painter.rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0, palette.orange),
            StrokeKind::Inside,
        );
        let grip_color = if active_zone == Some(zone) {
            palette.page
        } else {
            with_alpha(palette.orange, 156)
        };
        for offset in [-2.0, 2.0] {
            painter.line_segment(
                [
                    Pos2::new(rect.center().x - 2.0, rect.center().y + offset),
                    Pos2::new(rect.center().x + 2.0, rect.center().y + offset),
                ],
                Stroke::new(1.0, grip_color),
            );
        }
    }
}

pub(super) fn draw_horizontal_shift_cursor(
    painter: &egui::Painter,
    center: Pos2,
    palette: Palette,
) {
    let left = center + Vec2::new(-8.0, 0.0);
    let right = center + Vec2::new(8.0, 0.0);
    let stroke = Stroke::new(1.25, palette.orange);
    painter.line_segment([left, right], stroke);
    painter.line_segment([left, left + Vec2::new(3.5, -3.0)], stroke);
    painter.line_segment([left, left + Vec2::new(3.5, 3.0)], stroke);
    painter.line_segment([right, right + Vec2::new(-3.5, -3.0)], stroke);
    painter.line_segment([right, right + Vec2::new(-3.5, 3.0)], stroke);
    painter.circle_filled(center, 1.5, palette.page);
}

pub(super) fn draw_vertical_transform_cursor(
    painter: &egui::Painter,
    center: Pos2,
    palette: Palette,
) {
    let top = center + Vec2::new(0.0, -8.0);
    let bottom = center + Vec2::new(0.0, 8.0);
    let stroke = Stroke::new(1.25, palette.orange);
    painter.line_segment([top, bottom], stroke);
    painter.line_segment([top, top + Vec2::new(-3.0, 3.5)], stroke);
    painter.line_segment([top, top + Vec2::new(3.0, 3.5)], stroke);
    painter.line_segment([bottom, bottom + Vec2::new(-3.0, -3.5)], stroke);
    painter.line_segment([bottom, bottom + Vec2::new(3.0, -3.5)], stroke);
    painter.circle_filled(center, 1.5, palette.page);
}

pub(super) fn motion_curve_point(
    rect: Rect,
    context: &PlotContext<'_>,
    column: usize,
    columns: usize,
) -> (Pos2, Pos2, bool, bool) {
    let x = rect.left() + column as f32 / columns as f32 * rect.width();
    let frequency = x_to_frequency(x, rect, display_max_frequency(context.sample_rate));
    let active_bin = x_to_active_bin(x, rect, context.sample_rate, context.fft_size);
    let master_index = active_bin_to_master_index(active_bin, context.fft_size);
    let drawn_db = transformed_curve_db(
        context.base_curve,
        master_index,
        context.sample_rate,
        context.curve_depth_percent,
        context.curve_tilt_db_per_octave,
        context.curve_shift_semitones,
    );
    let moving_db = drawn_db.min(0.0) - displayed_motion_attenuation_db(frequency, context);
    (
        Pos2::new(x, curve_db_to_y(drawn_db, rect, context.display_range_db)),
        Pos2::new(x, curve_db_to_y(moving_db, rect, context.display_range_db)),
        drawn_db > 0.0,
        drawn_db < -context.display_range_db,
    )
}

pub(super) fn displayed_motion_attenuation_db(frequency: f32, context: &PlotContext<'_>) -> f32 {
    let splash = if context.motion.shape == MotionShape::Splash {
        splash_attenuation_db(
            frequency,
            context.motion.depth_db,
            context.motion.size_octaves,
            &context.splashes,
        )
    } else {
        0.0
    };
    motion_attenuation_db(frequency, context.motion) + splash
}

pub(super) fn draw_motion_outline(
    painter: &egui::Painter,
    rect: Rect,
    context: &PlotContext<'_>,
    trail: &mut LineTrail,
) {
    let drawn_columns = rect.width().ceil().max(2.0) as usize;
    let mut drawn = Vec::with_capacity(drawn_columns + 1);
    let mut above_ceiling = Vec::with_capacity(drawn_columns + 1);
    let mut below_range = Vec::with_capacity(drawn_columns + 1);
    for column in 0..=drawn_columns {
        let (drawn_point, _, is_above_ceiling, is_below_range) =
            motion_curve_point(rect, context, column, drawn_columns);
        drawn.push(drawn_point);
        above_ceiling.push(is_above_ceiling);
        below_range.push(is_below_range);
    }
    if context.motion.depth_db > 0.01 {
        let mut moving = Vec::with_capacity(MOTION_OUTLINE_POINTS + 1);
        for column in 0..=MOTION_OUTLINE_POINTS {
            let (_, moving_point, _, _) =
                motion_curve_point(rect, context, column, MOTION_OUTLINE_POINTS);
            moving.push(moving_point);
        }
        let (visibility, _) = note_line_visibilities(context.note_depth_db);
        trail.paint_and_capture(
            painter,
            &moving,
            context.motion.phase,
            TrailStyle {
                color: context.palette.orange,
                width: 1.0,
                visibility,
                active: visibility > 0.001,
            },
        );
        if visibility > 0.001 {
            painter.add(egui::Shape::line(
                moving,
                Stroke::new(
                    1.4,
                    with_alpha(context.palette.orange, (145.0 * visibility) as u8),
                ),
            ));
        }
        painter.add(egui::Shape::line(
            drawn,
            Stroke::new(2.2, context.palette.orange),
        ));
    } else {
        trail.paint_and_capture(
            painter,
            &[],
            context.motion.phase,
            TrailStyle {
                color: context.palette.orange,
                width: 1.0,
                visibility: 0.0,
                active: false,
            },
        );
        painter.add(egui::Shape::line(
            drawn,
            Stroke::new(2.2, context.palette.orange),
        ));
    }
    draw_curve_boundary_clips(
        painter,
        rect,
        &above_ceiling,
        rect.top() + 1.0,
        context.palette.warning,
    );
    draw_curve_boundary_clips(
        painter,
        rect,
        &below_range,
        rect.bottom() - 1.0,
        context.palette.warning,
    );
}

pub(super) fn draw_curve_boundary_clips(
    painter: &egui::Painter,
    rect: Rect,
    outside_range: &[bool],
    y: f32,
    color: Color32,
) {
    if outside_range.len() < 2 {
        return;
    }
    let columns = outside_range.len() - 1;
    let mut column = 0;
    while column <= columns {
        if !outside_range[column] {
            column += 1;
            continue;
        }
        let start = column;
        while column <= columns && outside_range[column] {
            column += 1;
        }
        let range_start = rect.left() + start as f32 / columns as f32 * rect.width();
        let range_end = rect.left() + column.min(columns) as f32 / columns as f32 * rect.width();
        let mut dash_start = range_start;
        while dash_start < range_end {
            let dash_end = (dash_start + 4.0).min(range_end);
            painter.line_segment(
                [Pos2::new(dash_start, y), Pos2::new(dash_end, y)],
                Stroke::new(1.2, color),
            );
            dash_start += 7.0;
        }
    }
}

pub(super) fn draw_note_skirts(
    painter: &egui::Painter,
    rect: Rect,
    context: &PlotContext<'_>,
    trail: &mut LineTrail,
) {
    let mut lobes = Vec::new();
    for note in 0..MIDI_NOTES {
        let note_level = context.display.note_level(note);
        if note_level <= 0.002 {
            continue;
        }
        let fundamental = note_frequency_with_tuning(note as u8, context.display.note_tuning(note));
        let rolloff = (context.partial_rolloff_db
            + (0.5 - context.display.note_timbre(note).clamp(0.0, 1.0)) * 24.0)
            .clamp(0.0, 48.0);
        for partial in 1..=context.partials {
            let center = fundamental * partial as f32;
            if center > display_max_frequency(context.sample_rate) {
                break;
            }
            let partial_gain = db_to_gain(-rolloff * (partial as f32).log2());
            lobes.push((center, note_level * partial_gain));
        }
    }

    let columns = rect.width().ceil().max(2.0) as usize;
    let mut outline = Vec::with_capacity(columns + 1);
    let mut openness_by_column = Vec::with_capacity(columns + 1);
    for column in 0..=columns {
        let x = rect.left() + column as f32 / columns as f32 * rect.width();
        let frequency = x_to_frequency(x, rect, display_max_frequency(context.sample_rate));
        let active_bin = x_to_active_bin(x, rect, context.sample_rate, context.fft_size);
        let master_index = active_bin_to_master_index(active_bin, context.fft_size);
        let ceiling_db = transformed_curve_db(
            context.base_curve,
            master_index,
            context.sample_rate,
            context.curve_depth_percent,
            context.curve_tilt_db_per_octave,
            context.curve_shift_semitones,
        )
        .min(0.0)
            - displayed_motion_attenuation_db(frequency, context);
        let ceiling_gain = db_to_gain(ceiling_db);
        let openness = lobes.iter().fold(0.0_f32, |maximum, (center, level)| {
            let cents = 1200.0 * (frequency / center).log2();
            let normalized = cents / context.note_width_cents.max(1.0);
            maximum.max(level * (-0.5 * normalized * normalized).exp())
        });
        let openness = openness.clamp(0.0, 1.0);
        // The blue plot communicates the selection shape, not the hidden
        // perceptual loudness compensation. A fully open note therefore meets
        // the orange ceiling even though the DSP may lift it by up to 6 dB.
        let visual_note_gate = visual_note_gate_gain(context.note_depth_db, openness);
        let gain = ceiling_gain * visual_note_gate;
        let db = if gain <= db_to_gain(MANUAL_CURVE_MUTE_DB) {
            MANUAL_CURVE_MUTE_DB
        } else {
            20.0 * gain.log10()
        };
        openness_by_column.push(openness);
        outline.push(Pos2::new(
            x,
            curve_db_to_y(db, rect, context.display_range_db),
        ));
    }

    let full_width = 1.6 + context.note_control_mix.clamp(0.0, 1.0) * 0.4;
    let (_, depth_fade) = note_line_visibilities(context.note_depth_db);
    let width = 0.7 + (full_width - 0.7) * depth_fade;
    trail.paint_and_capture(
        painter,
        &outline,
        context.motion.phase,
        TrailStyle {
            color: context.palette.blue,
            width: width.max(1.0),
            visibility: depth_fade,
            active: context.motion.depth_db > 0.01,
        },
    );
    painter.add(egui::Shape::line(
        outline.clone(),
        Stroke::new(
            width,
            with_alpha(context.palette.blue, (255.0 * depth_fade) as u8),
        ),
    ));

    // At zero depth, reveal held-note portions in blue even though the
    // uncompensated visual shape meets the unchanged orange ceiling. As depth
    // rises, the complete compound line above takes over smoothly.
    let note_alpha = (220.0 * (1.0 - depth_fade)) as u8;
    if note_alpha > 0 {
        let mut segment = Vec::new();
        for (point, openness) in outline.into_iter().zip(openness_by_column) {
            if openness > 0.002 {
                segment.push(point);
            } else if segment.len() >= 2 {
                painter.add(egui::Shape::line(
                    std::mem::take(&mut segment),
                    Stroke::new(width, with_alpha(context.palette.blue, note_alpha)),
                ));
            } else {
                segment.clear();
            }
        }
        if segment.len() >= 2 {
            painter.add(egui::Shape::line(
                segment,
                Stroke::new(width, with_alpha(context.palette.blue, note_alpha)),
            ));
        }
    }
}

pub(super) fn draw_visible_bin_boundaries(
    painter: &egui::Painter,
    rect: Rect,
    sample_rate: f32,
    fft_size: usize,
    palette: Palette,
) {
    let bin_hz = sample_rate / fft_size as f32;
    let first_bin = (MIN_DISPLAY_FREQUENCY_HZ / bin_hz).floor().max(1.0) as usize;
    let max_frequency = display_max_frequency(sample_rate);
    let last_bin = (max_frequency / bin_hz).ceil() as usize;
    for bin in first_bin..=last_bin.min(fft_size / 2) {
        let left_frequency = (bin as f32 - 0.5) * bin_hz;
        let right_frequency = (bin as f32 + 0.5) * bin_hz;
        let left = frequency_to_x(left_frequency, rect, max_frequency);
        let right = frequency_to_x(right_frequency, rect, max_frequency);
        if right - left < 2.25 {
            break;
        }
        painter.line_segment(
            [Pos2::new(left, rect.top()), Pos2::new(left, rect.bottom())],
            Stroke::new(1.0, with_alpha(palette.rule, 96)),
        );
    }
}

pub(super) fn draw_hovered_bin(
    painter: &egui::Painter,
    rect: Rect,
    pointer: Pos2,
    active_bin: usize,
    soft_pencil: bool,
    context: &PlotContext<'_>,
) {
    let sample_rate = context.sample_rate;
    let fft_size = context.fft_size;
    let palette = context.palette;
    let bin_hz = sample_rate / fft_size as f32;
    let frequency = active_bin as f32 * bin_hz;
    let max_frequency = display_max_frequency(sample_rate);
    let bin_left = frequency_to_x((active_bin as f32 - 0.5) * bin_hz, rect, max_frequency);
    let bin_right = frequency_to_x((active_bin as f32 + 0.5) * bin_hz, rect, max_frequency);
    if soft_pencil {
        painter.circle_filled(
            pointer,
            SOFT_PENCIL_RADIUS_PX,
            with_alpha(palette.orange, 14),
        );
        painter.circle_stroke(
            pointer,
            SOFT_PENCIL_RADIUS_PX,
            Stroke::new(1.0, with_alpha(palette.orange, 104)),
        );
        painter.circle_filled(pointer, 1.5, with_alpha(palette.orange, 176));
    } else {
        let highlight = Rect::from_min_max(
            Pos2::new(bin_left.min(bin_right - 1.0), rect.top()),
            Pos2::new(bin_right.max(bin_left + 1.0), rect.bottom()),
        );
        painter.rect_filled(highlight, 0.0, with_alpha(palette.orange, 24));
        let edge_stroke = Stroke::new(1.0, with_alpha(palette.orange, 112));
        painter.line_segment(
            [
                Pos2::new(bin_left, rect.top()),
                Pos2::new(bin_left, rect.bottom()),
            ],
            edge_stroke,
        );
        painter.line_segment(
            [
                Pos2::new(bin_right, rect.top()),
                Pos2::new(bin_right, rect.bottom()),
            ],
            edge_stroke,
        );
        draw_pen_cursor(painter, pointer, palette);
    }
    painter.text(
        Pos2::new(rect.left() + 126.0, rect.top() + 4.0),
        Align2::LEFT_TOP,
        format!(
            "{}BIN {active_bin}  ·  {frequency:.1} Hz  ·  {}",
            if soft_pencil { "SMOOTH  ·  " } else { "" },
            frequency_note_label(frequency),
        ),
        FontId::monospace(oiko_ui::typography::TEXT_SMALL),
        palette.muted,
    );
}

pub(super) fn draw_pen_cursor(painter: &egui::Painter, tip: Pos2, palette: Palette) {
    let direction = Vec2::new(0.707, -0.707);
    let perpendicular = Vec2::new(0.707, 0.707);
    let shoulder = tip + direction * 2.5;
    let end = tip + direction * 13.0;
    let half_width = 1.8;
    let points = vec![
        tip,
        shoulder + perpendicular * half_width,
        end + perpendicular * half_width,
        end - perpendicular * half_width,
        shoulder - perpendicular * half_width,
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        palette.page,
        Stroke::new(1.0, palette.orange),
    ));
    painter.line_segment(
        [
            end + perpendicular * half_width,
            end - perpendicular * half_width,
        ],
        Stroke::new(1.0, palette.orange),
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_harmonic_preview(
    painter: &egui::Painter,
    plot: Rect,
    fundamental: f32,
    partials: usize,
    rolloff: f32,
    sample_rate: f32,
    palette: Palette,
) {
    let painter = painter.with_clip_rect(plot);
    for partial in 1..=partials {
        let frequency = fundamental * partial as f32;
        if frequency > display_max_frequency(sample_rate) {
            break;
        }
        let x = frequency_to_x(frequency, plot, display_max_frequency(sample_rate));
        let strength = db_to_gain(-rolloff * (partial as f32).log2());
        painter.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.top() + 10.0)],
            Stroke::new(1.0, with_alpha(palette.blue, (150.0 * strength) as u8)),
        );
        painter.line_segment(
            [Pos2::new(x, plot.top() + 10.0), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0, with_alpha(palette.blue, (25.0 * strength) as u8)),
        );
    }
}

pub(super) fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub(super) fn clamp_to_rect(point: Pos2, rect: Rect) -> Pos2 {
    Pos2::new(
        point.x.clamp(rect.left(), rect.right()),
        point.y.clamp(rect.top(), rect.bottom()),
    )
}

pub(super) fn mix_color(from: Color32, to: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * amount) as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

pub(super) fn ruler_intro_strength(position: f32, elapsed_seconds: f32) -> f32 {
    if !(0.0..RULER_INTRO_DURATION_SECONDS).contains(&elapsed_seconds) {
        return 0.0;
    }
    let progress = elapsed_seconds / RULER_INTRO_DURATION_SECONDS;
    let head = -0.10 + progress * 1.20;
    let distance = position.clamp(0.0, 1.0) - head;
    let front = (-0.5 * (distance / 0.035).powi(2)).exp();
    let wake_distance = -distance;
    let wake = if (0.0..=0.28).contains(&wake_distance) {
        let ripple = (PI * wake_distance / 0.07).sin().powi(2);
        ripple * (-wake_distance * 7.0).exp()
    } else {
        0.0
    };
    (front * 0.72 + wake * 0.42).clamp(0.0, 1.0)
}

pub(super) fn note_line_visibilities(note_depth_db: f32) -> (f32, f32) {
    let position = (note_depth_db / 10.0).clamp(0.0, 1.0);
    let crossfade = position * position * (3.0 - 2.0 * position);
    let orange = 1.0 - crossfade;
    let blue = crossfade.powf(1.08);
    (orange, blue)
}

pub(super) fn visual_note_gate_gain(note_depth_db: f32, openness: f32) -> f32 {
    let floor = db_to_gain(-note_depth_db.max(0.0));
    floor + (1.0 - floor) * openness.clamp(0.0, 1.0)
}

pub(super) fn x_to_active_bin(x: f32, rect: Rect, sample_rate: f32, fft_size: usize) -> usize {
    let frequency = x_to_frequency(x, rect, display_max_frequency(sample_rate));
    ((frequency * fft_size as f32 / sample_rate).round() as usize).min(fft_size / 2)
}

pub(super) fn active_bin_to_master_index(active_bin: usize, fft_size: usize) -> usize {
    let active_bins = fft_size / 2;
    (active_bin * (MANUAL_MASK_POINTS - 1) / active_bins).min(MANUAL_MASK_POINTS - 1)
}

pub(super) fn active_bin_master_span(active_bin: usize, fft_size: usize) -> (usize, usize) {
    let first = active_bin_to_master_index(active_bin, fft_size);
    let last = if active_bin >= fft_size / 2 {
        MANUAL_MASK_POINTS - 1
    } else {
        active_bin_to_master_index(active_bin + 1, fft_size).saturating_sub(1)
    };
    (first, last.max(first))
}

pub(super) fn x_to_frequency(x: f32, rect: Rect, max_frequency: f32) -> f32 {
    let normalized = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    MIN_DISPLAY_FREQUENCY_HZ * (max_frequency / MIN_DISPLAY_FREQUENCY_HZ).powf(normalized)
}

pub(super) fn frequency_note_label(frequency: f32) -> String {
    if frequency <= 0.0 {
        return "—".to_owned();
    }
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    let fractional_note = 69.0 + 12.0 * (frequency / 440.0).log2();
    let note = fractional_note.round().clamp(0.0, 127.0) as i32;
    let cents = ((fractional_note - note as f32) * 100.0).round() as i32;
    let octave = midi_note_octave(note);
    format!("{}{} {cents:+}¢", NAMES[note as usize % 12], octave)
}

pub(super) fn midi_note_octave(note: i32) -> i32 {
    note / 12 - 2
}

pub(super) fn draw_spectrum(painter: &egui::Painter, rect: Rect, context: &PlotContext<'_>) {
    let note_control_mix = context.note_control_mix.clamp(0.0, 1.0);
    let spectrum_color = mix_color(context.palette.blue, context.palette.muted, 0.30);
    let fill_alpha = (30.0 - note_control_mix * 14.0) as u8;
    let outline_alpha = (220.0 - note_control_mix * 75.0) as u8;
    let outline_width = 1.3 - note_control_mix * 0.2;
    let max_frequency = display_max_frequency(context.sample_rate);
    let bin_hz = context.sample_rate / context.fft_size as f32;
    let columns = rect.width().ceil().max(2.0) as usize;
    let mut outline = Vec::with_capacity(columns * 2);
    let mut previous_bin = None;
    let mut previous_y = rect.bottom();
    for column in 0..=columns {
        let x = rect.left() + column as f32 / columns as f32 * rect.width();
        let frequency = x_to_frequency(x, rect, max_frequency);
        let active_bin = ((frequency / bin_hz).round() as usize).min(context.fft_size / 2);
        let bin_frequency =
            (active_bin as f32 * bin_hz).clamp(MIN_DISPLAY_FREQUENCY_HZ, max_frequency);
        let db = analyzer_db_at_frequency(context.spectrum_db, bin_frequency, max_frequency);
        let y = db_to_y(db, rect, context.display_range_db);
        if previous_bin.is_some_and(|previous| previous != active_bin) {
            let lower = (active_bin as f32 - 0.5).max(0.0) * bin_hz;
            let upper = (active_bin as f32 + 0.5) * bin_hz;
            let visible_bin_width = frequency_to_x(upper, rect, max_frequency)
                - frequency_to_x(lower.max(MIN_DISPLAY_FREQUENCY_HZ), rect, max_frequency);
            if visible_bin_width >= 1.5 {
                outline.push(Pos2::new(x, previous_y));
            }
        }
        outline.push(Pos2::new(x, y));
        previous_bin = Some(active_bin);
        previous_y = y;
    }
    let mut fill = egui::Mesh::default();
    let fill_color = with_alpha(spectrum_color, fill_alpha);
    for point in &outline {
        fill.colored_vertex(*point, fill_color);
        fill.colored_vertex(Pos2::new(point.x, rect.bottom()), fill_color);
    }
    for index in 0..outline.len().saturating_sub(1) {
        let top_left = (index * 2) as u32;
        let bottom_left = top_left + 1;
        let top_right = top_left + 2;
        let bottom_right = top_left + 3;
        fill.add_triangle(top_left, bottom_left, top_right);
        fill.add_triangle(top_right, bottom_left, bottom_right);
    }
    painter.add(egui::Shape::mesh(fill));
    painter.add(egui::Shape::line(
        outline,
        Stroke::new(outline_width, with_alpha(spectrum_color, outline_alpha)),
    ));
}

pub(super) fn analyzer_db_at_frequency(
    spectrum_db: &[f32; ANALYZER_POINTS],
    frequency: f32,
    max_frequency: f32,
) -> f32 {
    let normalized =
        (frequency.clamp(MIN_DISPLAY_FREQUENCY_HZ, max_frequency) / MIN_DISPLAY_FREQUENCY_HZ).ln()
            / (max_frequency / MIN_DISPLAY_FREQUENCY_HZ).ln();
    let position = normalized * (ANALYZER_POINTS - 1) as f32;
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(ANALYZER_POINTS - 1);
    let fraction = position - lower as f32;
    spectrum_db[lower] + (spectrum_db[upper] - spectrum_db[lower]) * fraction
}

pub(super) fn draw_grid_lines(
    painter: &egui::Painter,
    rect: Rect,
    sample_rate: f32,
    display_range_db: f32,
    palette: Palette,
) {
    let grid = palette.rule;
    let max_frequency = display_max_frequency(sample_rate);
    for division in 0..=6 {
        let db = -(division as f32 / 6.0) * display_range_db;
        let y = db_to_y(db, rect, display_range_db);
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0, grid),
        );
    }

    for frequency in [
        50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0,
    ]
    .into_iter()
    .filter(|frequency| *frequency < max_frequency - 1.0)
    {
        let x = frequency_to_x(frequency, rect, max_frequency);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, grid),
        );
    }
}

pub(super) fn draw_axis_veil(painter: &egui::Painter, rect: Rect, palette: Palette) {
    let strong = with_alpha(palette.page, 210);
    let clear = with_alpha(palette.page, 0);
    let mut mesh = egui::Mesh::default();
    let graph_bounds = painter.clip_rect();
    let outer_left = graph_bounds.left();
    let outer_right = graph_bounds.right();
    let outer_top = graph_bounds.top();
    let outer_bottom = graph_bounds.bottom();

    let left_edge = mesh.vertices.len() as u32;
    mesh.colored_vertex(Pos2::new(outer_left, outer_top), strong);
    mesh.colored_vertex(Pos2::new(outer_left, outer_bottom), strong);
    mesh.colored_vertex(Pos2::new(rect.left() + 36.0, outer_bottom), clear);
    mesh.colored_vertex(Pos2::new(rect.left() + 36.0, outer_top), clear);
    mesh.add_triangle(left_edge, left_edge + 1, left_edge + 2);
    mesh.add_triangle(left_edge, left_edge + 2, left_edge + 3);

    let bottom_edge = mesh.vertices.len() as u32;
    mesh.colored_vertex(Pos2::new(rect.left(), rect.bottom() - 17.0), clear);
    mesh.colored_vertex(Pos2::new(rect.left(), outer_bottom), strong);
    mesh.colored_vertex(Pos2::new(rect.right(), outer_bottom), strong);
    mesh.colored_vertex(Pos2::new(rect.right(), rect.bottom() - 17.0), clear);
    mesh.add_triangle(bottom_edge, bottom_edge + 1, bottom_edge + 2);
    mesh.add_triangle(bottom_edge, bottom_edge + 2, bottom_edge + 3);

    let right_edge = mesh.vertices.len() as u32;
    mesh.colored_vertex(Pos2::new(rect.right() - 36.0, outer_top), clear);
    mesh.colored_vertex(Pos2::new(rect.right() - 36.0, outer_bottom), clear);
    mesh.colored_vertex(Pos2::new(outer_right, outer_bottom), strong);
    mesh.colored_vertex(Pos2::new(outer_right, outer_top), strong);
    mesh.add_triangle(right_edge, right_edge + 1, right_edge + 2);
    mesh.add_triangle(right_edge, right_edge + 2, right_edge + 3);

    let top_edge = mesh.vertices.len() as u32;
    mesh.colored_vertex(Pos2::new(rect.left(), outer_top), strong);
    mesh.colored_vertex(Pos2::new(rect.left(), rect.top() + 17.0), clear);
    mesh.colored_vertex(Pos2::new(rect.right(), rect.top() + 17.0), clear);
    mesh.colored_vertex(Pos2::new(rect.right(), outer_top), strong);
    mesh.add_triangle(top_edge, top_edge + 1, top_edge + 2);
    mesh.add_triangle(top_edge, top_edge + 2, top_edge + 3);

    painter.add(egui::Shape::mesh(mesh));
}

pub(super) fn draw_axis_labels(
    painter: &egui::Painter,
    rect: Rect,
    sample_rate: f32,
    display_range_db: f32,
    palette: Palette,
) {
    let label = palette.muted;
    let max_frequency = display_max_frequency(sample_rate);
    for division in 0..=6 {
        let db = -(division as f32 / 6.0) * display_range_db;
        let y = db_to_y(db, rect, display_range_db);
        painter.text(
            Pos2::new(rect.left() + 3.0, y - 2.0),
            Align2::LEFT_BOTTOM,
            axis_db_label(db, division, display_range_db),
            FontId::monospace(oiko_ui::typography::TEXT_SMALL),
            label,
        );
    }

    for (frequency, text) in [
        (50.0, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1000.0, "1k"),
        (2000.0, "2k"),
        (5000.0, "5k"),
        (10_000.0, "10k"),
        (20_000.0, "20k"),
    ]
    .into_iter()
    .filter(|(frequency, _)| *frequency < max_frequency - 1.0)
    {
        let x = frequency_to_x(frequency, rect, max_frequency);
        painter.text(
            Pos2::new(x + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            text,
            FontId::monospace(oiko_ui::typography::TEXT_SMALL),
            label,
        );
    }
}

pub(super) fn draw_plot_labels(
    painter: &egui::Painter,
    rect: Rect,
    note_control_mix: f32,
    palette: Palette,
) {
    let note_control_mix = note_control_mix.clamp(0.0, 1.0);
    let spectrum_color = mix_color(palette.blue, palette.muted, 0.30);
    let outline_alpha = (220.0 - note_control_mix * 75.0) as u8;
    painter.text(
        Pos2::new(rect.right() - 4.0, rect.top() - 2.0),
        Align2::RIGHT_BOTTOM,
        "SPECTRAL GAIN",
        FontId::monospace(oiko_ui::typography::TEXT_SMALL),
        palette.orange,
    );
    painter.text(
        Pos2::new(rect.right() - 4.0, rect.bottom() + 2.0),
        Align2::RIGHT_TOP,
        "INPUT SPECTRUM",
        FontId::monospace(oiko_ui::typography::TEXT_SMALL),
        with_alpha(spectrum_color, outline_alpha.max(170)),
    );
}

pub(super) fn draw_input_notes(
    painter: &egui::Painter,
    rect: Rect,
    display: &AnalysisDisplay,
    sample_rate: f32,
    palette: Palette,
) {
    let max_frequency = display_max_frequency(sample_rate);
    for note in 0..MIDI_NOTES {
        let level = display.note_level(note);
        if level <= 0.002 {
            continue;
        }
        let frequency = note_frequency_with_tuning(note as u8, display.note_tuning(note));
        if frequency < MIN_DISPLAY_FREQUENCY_HZ || frequency > max_frequency {
            continue;
        }
        let x = frequency_to_x(frequency, rect, max_frequency);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(
                0.9 + level * 0.9,
                with_alpha(palette.blue, (35.0 + level * 145.0) as u8),
            ),
        );
    }
}

pub(super) fn frequency_to_x(frequency: f32, rect: Rect, max_frequency: f32) -> f32 {
    let normalized =
        (frequency.clamp(MIN_DISPLAY_FREQUENCY_HZ, max_frequency) / MIN_DISPLAY_FREQUENCY_HZ).ln()
            / (max_frequency / MIN_DISPLAY_FREQUENCY_HZ).ln();
    rect.left() + normalized * rect.width()
}

pub(super) fn db_to_y(db: f32, rect: Rect, display_range_db: f32) -> f32 {
    let range = display_range_db.clamp(30.0, 144.0);
    rect.top() + (-db.clamp(-range, 0.0) / range) * rect.height()
}

pub(super) fn curve_db_to_y(db: f32, rect: Rect, display_range_db: f32) -> f32 {
    let range = display_range_db.clamp(30.0, 144.0);
    rect.top() + (-db / range) * rect.height()
}

pub(super) fn y_to_curve_db(y: f32, rect: Rect, display_range_db: f32) -> f32 {
    let range = display_range_db.clamp(30.0, 144.0);
    (-(y - rect.top()) / rect.height() * range).clamp(-range, 0.0)
}

pub(super) fn axis_db_label(db: f32, division: usize, display_range_db: f32) -> String {
    if division == 6 && (display_range_db - 144.0).abs() < 0.1 {
        "−∞".to_owned()
    } else {
        format!("{db:.0}")
    }
}

#[cfg(test)]
mod tests;
