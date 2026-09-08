use super::*;
use super::{controls::*, curve::*, history::*, rendering::*};
use crate::display_data::display_max_frequency;
use crate::parameters::SpectralParams;
use crate::state::CurveState;
use oiko_dsp::db_to_gain;

fn flat_curve_snapshot() -> CurveSnapshot {
    CurveSnapshot::capture(&[0.0; MANUAL_MASK_POINTS], &SpectralParams::default())
}

#[test]
fn active_bin_spans_cover_the_master_mask_without_gaps() {
    for fft_size in [1024, 2048, 4096, 8192, 16_384] {
        let mut expected_first = 0;
        for bin in 0..=fft_size / 2 {
            let (first, last) = active_bin_master_span(bin, fft_size);
            assert_eq!(first, expected_first);
            assert!(last >= first);
            expected_first = last + 1;
        }
        assert_eq!(expected_first, MANUAL_MASK_POINTS);
    }
}

#[test]
fn exact_pencil_preserves_sharp_adjacent_bin_changes() {
    let curve = CurveState::default();
    edit_with_bins(
        &curve,
        Some(EditPoint {
            active_bin: 40,
            db: 0.0,
        }),
        EditPoint {
            active_bin: 41,
            db: -60.0,
        },
        1024,
    );
    assert_eq!(curve.get(active_bin_to_master_index(40, 1024)), 0.0);
    assert_eq!(curve.get(active_bin_to_master_index(41, 1024)), -60.0);
}

#[test]
fn soft_pencil_has_a_smooth_radial_falloff() {
    let curve = CurveState::default();
    edit_with_soft_bins(
        &curve,
        None,
        EditPoint {
            active_bin: 40,
            db: -60.0,
        },
        1024,
        4,
    );
    let center = curve.get(active_bin_to_master_index(40, 1024));
    let near = curve.get(active_bin_to_master_index(41, 1024));
    let outer = curve.get(active_bin_to_master_index(43, 1024));
    let distant = curve.get(active_bin_to_master_index(48, 1024));
    assert_eq!(center, -60.0);
    assert!(near > center && near < -45.0);
    assert!(outer > near && outer < 0.0);
    assert_eq!(distant, 0.0);
}

#[test]
fn midi_octave_numbering_uses_middle_c_as_c3() {
    assert_eq!(midi_note_octave(60), 3);
    assert_eq!(midi_note_octave(72), 4);
}

#[test]
fn display_caps_at_22khz_in_high_sample_rate_sessions() {
    assert_eq!(display_max_frequency(44_100.0), 22_000.0);
    assert_eq!(display_max_frequency(48_000.0), 22_000.0);
    assert_eq!(display_max_frequency(96_000.0), 22_000.0);
    assert_eq!(display_max_frequency(32_000.0), 16_000.0);
}

#[test]
fn note_plot_omits_hidden_loudness_compensation() {
    let plot = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(750.0, 290.0));
    assert_eq!(visual_note_gate_gain(0.0, 0.0), 1.0);
    assert_eq!(visual_note_gate_gain(0.0, 1.0), 1.0);
    assert_eq!(visual_note_gate_gain(24.0, 1.0), 1.0);
    assert!((visual_note_gate_gain(24.0, 0.0) - db_to_gain(-24.0)).abs() < 1.0e-6);
    assert_eq!(db_to_y(0.0, plot, 90.0), plot.top());
    assert_eq!(db_to_y(-90.0, plot, 90.0), plot.bottom());
    assert_eq!(
        db_to_y(-60.0, plot, 90.0),
        plot.top() + plot.height() * 2.0 / 3.0
    );
    assert_eq!(y_to_curve_db(plot.bottom(), plot, 90.0), -90.0);
    assert_eq!(y_to_curve_db(plot.bottom(), plot, 144.0), -144.0);
}

#[test]
fn graph_range_sets_drawing_depth() {
    let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(600.0, 300.0));
    assert_eq!(y_to_curve_db(plot.center().y, plot, 30.0), -15.0);
    assert_eq!(y_to_curve_db(plot.center().y, plot, 60.0), -30.0);
    assert_eq!(y_to_curve_db(plot.center().y, plot, 90.0), -45.0);
    assert_eq!(y_to_curve_db(plot.bottom(), plot, 30.0), -30.0);
    assert_eq!(y_to_curve_db(plot.bottom(), plot, 60.0), -60.0);
    assert_eq!(y_to_curve_db(plot.bottom(), plot, 90.0), -90.0);
    assert_eq!(y_to_curve_db(plot.bottom(), plot, 144.0), -144.0);
}

#[test]
fn curve_depth_drag_reaches_zero_and_eight_hundred_percent_in_one_gesture() {
    assert_eq!(curve_depth_from_drag(100.0, 0.0, 300.0), 100.0);
    assert_eq!(curve_depth_from_drag(100.0, -300.0, 300.0), 0.0);
    assert!((curve_depth_from_drag(100.0, 300.0, 300.0) - 800.0).abs() < 1.0e-4);
}

#[test]
fn capture_normalizes_a_flat_average_to_the_zero_db_ceiling() {
    let curve = CurveState::default();
    let power = [0.25_f64; ANALYZER_POINTS];
    assert!(capture_spectrum_to_curve(&power, 4, &curve, 48_000.0, 2048,));
    for index in [0, MANUAL_MASK_POINTS / 2, MANUAL_MASK_POINTS - 1] {
        assert_eq!(curve.get(index), 0.0);
    }
}

#[test]
fn flip_preserves_the_occupied_range_including_hidden_values() {
    let curve = CurveState::default();
    curve.set_transformed(0, 6.0);
    curve.set_transformed(1, 0.0);
    curve.set_transformed(2, -12.0);
    curve.set_transformed(3, -48.0);
    let mut source = [0.0; MANUAL_MASK_POINTS];
    curve.copy_to(&mut source);

    assert!(flip_curve(&source, &curve));
    assert_eq!(curve.get(0), -48.0);
    assert_eq!(curve.get(1), -42.0);
    assert_eq!(curve.get(2), -30.0);
    assert_eq!(curve.get(3), 6.0);
    let original = source;
    curve.copy_to(&mut source);
    assert!(flip_curve(&source, &curve));
    curve.copy_to(&mut source);
    assert_eq!(source, original);
}

#[test]
fn flip_leaves_a_flat_curve_unchanged() {
    let curve = CurveState::default();
    let source = [-18.0; MANUAL_MASK_POINTS];

    assert!(!flip_curve(&source, &curve));
    assert_eq!(curve.get(0), 0.0);
}

#[test]
fn curve_history_restores_whole_curve_gestures() {
    let curve = CurveState::default();
    let mut history = CurveHistory::default();
    let mut snapshot = [0.0; MANUAL_MASK_POINTS];

    curve.copy_to(&mut snapshot);
    history.record_before(flat_curve_snapshot());
    curve.set(42, -18.0);

    curve.copy_to(&mut snapshot);
    let current = CurveSnapshot::capture(&snapshot, &SpectralParams::default());
    let HistoryEntry::Curve(previous) = history
        .undo(current, &SpectralParams::default())
        .expect("undo snapshot")
    else {
        panic!("curve entry")
    };
    restore_curve(&curve, &previous.curve);
    assert_eq!(curve.get(42), 0.0);

    curve.copy_to(&mut snapshot);
    let current = CurveSnapshot::capture(&snapshot, &SpectralParams::default());
    let HistoryEntry::Curve(next) = history
        .redo(current, &SpectralParams::default())
        .expect("redo snapshot")
    else {
        panic!("curve entry")
    };
    restore_curve(&curve, &next.curve);
    assert_eq!(curve.get(42), -18.0);
}

#[test]
fn curve_history_restores_hidden_transform_values() {
    let curve = CurveState::default();
    curve.set_transformed(42, 9.25);
    let mut snapshot = [0.0; MANUAL_MASK_POINTS];
    curve.copy_to(&mut snapshot);

    let restored = CurveState::default();
    restore_curve(&restored, &snapshot);
    assert!((restored.get(42) - 9.25).abs() < 1.0e-6);
}

#[test]
fn applied_transform_is_one_undoable_curve_gesture() {
    let curve = CurveState::default();
    let mut history = CurveHistory::default();
    curve.set(42, -24.0);
    let mut source = [0.0; MANUAL_MASK_POINTS];
    curve.copy_to(&mut source);
    let before = CurveSnapshot::capture(&source, &SpectralParams::default());
    history.record_before(before.clone());
    apply_curve_transform(&source, &curve, 48_000.0, 50.0, 0.0, 0.0);
    assert_eq!(curve.get(42), -12.0);

    curve.copy_to(&mut source);
    let after = CurveSnapshot::capture(&source, &SpectralParams::default());
    let HistoryEntry::Curve(restored) = history
        .undo(after.clone(), &SpectralParams::default())
        .expect("undo transform")
    else {
        panic!("curve entry")
    };
    assert_eq!(restored.depth_percent, 100.0);
    assert_eq!(restored.curve[42], -24.0);

    let HistoryEntry::Curve(restored) = history
        .redo(before, &SpectralParams::default())
        .expect("redo transform")
    else {
        panic!("curve entry")
    };
    assert_eq!(restored.depth_percent, 100.0);
    assert_eq!(restored.curve[42], -12.0);
}

#[test]
fn command_z_selects_curve_undo_when_history_is_available() {
    let context = egui::Context::default();
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        ..Default::default()
    });
    let mut history = CurveHistory::default();
    history.record_before(flat_curve_snapshot());

    assert!(matches!(
        curve_history_shortcut(&context, &history, true),
        Some(CurveAction::Undo)
    ));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn space_is_always_left_for_the_host() {
    assert_eq!(
        host_key_capture(true, false),
        nice_plug_egui::KeyCapture::CaptureCommands(vec![
            nice_plug_egui::Key::Character("z".into()),
            nice_plug_egui::Key::Character("Z".into()),
        ])
    );
}

#[test]
fn undo_keys_are_left_for_the_host_when_curve_shortcuts_are_inactive() {
    assert_eq!(
        host_key_capture(false, false),
        nice_plug_egui::KeyCapture::IgnoreAll
    );

    let context = egui::Context::default();
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        ..Default::default()
    });
    let mut history = CurveHistory::default();
    history.record_before(flat_curve_snapshot());

    assert!(curve_history_shortcut(&context, &history, false).is_none());
    assert!(context.input(|input| input.key_pressed(Key::Z)));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn numeric_text_edit_temporarily_captures_keyboard_input() {
    assert_eq!(
        host_key_capture(false, true),
        nice_plug_egui::KeyCapture::CaptureAll
    );
}

#[test]
fn escape_is_consumed_only_for_a_temporary_mode() {
    let context = egui::Context::default();
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    });

    assert!(!consume_escape_for_temporary_mode(&context, false));
    assert!(context.input(|input| input.key_pressed(Key::Escape)));
    assert!(consume_escape_for_temporary_mode(&context, true));
    assert!(!context.input(|input| input.key_pressed(Key::Escape)));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn alt_immediately_previews_transform_without_latching_it() {
    assert!(transform_interaction_active(
        false, false, true, false, false
    ));
    assert!(!transform_interaction_active(
        false, false, false, false, false
    ));
    assert!(transform_interaction_active(
        false, false, false, true, false
    ));
    assert!(!transform_interaction_active(true, true, true, true, false));
    assert!(!transform_interaction_active(
        false, false, true, true, true
    ));
}

#[test]
fn transform_edge_zones_are_large_and_bottom_takes_corner_priority() {
    let frame = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(210.0, 120.0));
    assert_eq!(
        curve_transform_zone(Pos2::new(15.0, 50.0), frame),
        CurveTransformZone::TiltLow
    );
    assert_eq!(
        curve_transform_zone(Pos2::new(205.0, 50.0), frame),
        CurveTransformZone::TiltHigh
    );
    assert_eq!(
        curve_transform_zone(Pos2::new(110.0, 110.0), frame),
        CurveTransformZone::Depth
    );
    assert_eq!(
        curve_transform_zone(Pos2::new(15.0, 110.0), frame),
        CurveTransformZone::Depth
    );
    assert_eq!(
        curve_transform_zone(Pos2::new(110.0, 50.0), frame),
        CurveTransformZone::Shift
    );
}

#[test]
fn command_shift_z_selects_curve_redo_when_history_is_available() {
    let context = egui::Context::default();
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        }],
        ..Default::default()
    });
    let mut history = CurveHistory::default();
    let before = flat_curve_snapshot();
    let mut after = before.clone();
    after.curve[42] = -18.0;
    history.record_before(before);
    let _ = history.undo(after, &SpectralParams::default());

    assert!(matches!(
        curve_history_shortcut(&context, &history, true),
        Some(CurveAction::Redo)
    ));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn exhausted_redo_does_not_fall_through_to_undo() {
    let context = egui::Context::default();
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        }],
        ..Default::default()
    });
    let mut history = CurveHistory::default();
    history.record_before(flat_curve_snapshot());

    assert!(curve_history_shortcut(&context, &history, true).is_none());
    assert!(!context.input(|input| input.key_pressed(Key::Z)));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn exhausted_undo_stays_local_while_redo_is_available() {
    let context = egui::Context::default();
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        ..Default::default()
    });
    let mut history = CurveHistory::default();
    let before = flat_curve_snapshot();
    let mut after = before.clone();
    after.depth_percent = 43.0;
    history.record_before(before);
    let _ = history.undo(after, &SpectralParams::default());

    assert!(curve_history_shortcut(&context, &history, true).is_none());
    assert!(!context.input(|input| input.key_pressed(Key::Z)));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn unavailable_curve_shortcut_is_left_for_the_host() {
    let context = egui::Context::default();
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        ..Default::default()
    });

    assert!(curve_history_shortcut(&context, &CurveHistory::default(), true).is_none());
    assert!(context.input(|input| input.key_pressed(Key::Z)));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn focused_non_text_control_does_not_block_curve_shortcuts() {
    let context = egui::Context::default();
    context.memory_mut(|memory| memory.request_focus(Id::new("curve-surface")));
    context.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        ..Default::default()
    });
    let mut history = CurveHistory::default();
    history.record_before(flat_curve_snapshot());

    assert!(matches!(
        curve_history_shortcut(&context, &history, true),
        Some(CurveAction::Undo)
    ));
    let mut output = context.end_pass();
    output.textures_delta.clear();
}

#[test]
fn applied_tilt_preserves_hidden_curve_shape() {
    let curve = CurveState::default();
    let source = [0.0; MANUAL_MASK_POINTS];
    apply_curve_transform(&source, &curve, 48_000.0, 100.0, 6.0, 0.0);

    let two_khz = (2_000.0 / 24_000.0 * (MANUAL_MASK_POINTS - 1) as f32).round() as usize;
    let four_khz = (4_000.0 / 24_000.0 * (MANUAL_MASK_POINTS - 1) as f32).round() as usize;
    assert!(curve.get(two_khz) > 0.0);
    assert!(curve.get(four_khz) > curve.get(two_khz));
}

#[test]
fn effective_note_transition_tracks_resolution_and_sample_rate() {
    assert!((estimated_note_transition_ms(1024, 48_000.0) - 8.0).abs() < 1.0e-6);
    assert!((estimated_note_transition_ms(8192, 48_000.0) - 64.0).abs() < 1.0e-6);
    assert!(estimated_note_transition_ms(8192, 44_100.0) > 64.0);
}

#[test]
fn note_and_motion_lines_crossfade_within_the_low_depth_range() {
    assert_eq!(note_line_visibilities(0.0), (1.0, 0.0));
    let (orange, blue) = note_line_visibilities(5.0);
    assert!((orange - 0.5).abs() < 1.0e-6);
    assert!(blue > 0.45 && blue < orange);
    assert_eq!(note_line_visibilities(10.0), (0.0, 1.0));
    assert_eq!(note_line_visibilities(90.0), (0.0, 1.0));
}

#[test]
fn window_open_applies_zoom_restored_after_editor_creation() {
    let params = Arc::new(SpectralParams::default());
    let editor = SpectralEditor::new(params.clone(), Arc::new(AnalysisDisplay::default()));
    let canvas = Vec2::new(EDITOR_WIDTH, EDITOR_HEIGHT);

    // Reuse the editor, as hosts do, and restore settings before each window opens.
    for scale in [1.25, 1.0, 1.5, 0.75, 1.25] {
        params.ui_scale.set(scale);
        let context = egui::Context::default();
        editor.initialize_view(&context);
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, canvas)),
                ..Default::default()
            },
            |_| {},
        );
        output.textures_delta.clear();
        if cfg!(target_os = "macos") {
            let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
            assert!(commands.iter().any(|command| {
                matches!(command, egui::ViewportCommand::InnerSize(size) if *size == canvas * scale)
            }), "opening at {scale} must request matching window geometry");
            assert_eq!(context.zoom_factor(), 1.0);
        } else {
            assert_eq!(context.zoom_factor(), scale);
        }
    }
}
