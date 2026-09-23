use super::*;
use crate::parameters::FftQuality;
use crate::parameters::MotionDirection;
use crate::parameters::MotionRateDivision;
use crate::parameters::SpectralMotionShape;
use crate::parameters::SpectralParams;
use spectral_dsp::particles::{Frame, MAX_PARTICLES};

fn select(p: &mut SpectralPlugin, shape: SpectralMotionShape) {
    unsafe {
        p.params.motion_shape._internal_set_plain_value(shape);
        p.params.motion_sync._internal_set_plain_value(false);
        p.params.motion_depth_db._internal_set_plain_value(24.0);
        p.params.motion_rate_hz._internal_set_plain_value(13.0);
        p.params.smooth_spectral._internal_set_plain_value(false);
    }
    for (_, parameter, _) in p.params.param_map() {
        unsafe {
            parameter._internal_update_smoother(p.sample_rate, true);
        }
    }
}
fn frame(p: &mut SpectralPlugin) -> Frame {
    p.particles
        .engine
        .frame(p.sample_rate / p.quality.size() as f32, 0.0)
}

#[test]
fn initial_expression_precedence_and_logical_note_age_reach_particles() {
    let (mut p, mut c) = setup();
    select(&mut p, SpectralMotionShape::Sprinkle);
    c.events.extend([tuning(137, 1, 3.25), note(137, 1)]);
    process(&mut p, &mut c, 0, 138);
    assert_eq!(p.particles.engine.elapsed_samples(), 138);
    assert_eq!(p.particles.engine.births, 1);
    let f = frame(&mut p);
    let window = f.windows.iter().find(|w| w.amplitude > 0.0).unwrap();
    let fundamental = oiko_dsp::note_frequency(60) * 2.0_f32.powf(3.25 / 12.0);
    let octave = window.center_log2_hz - fundamental.log2();
    assert!((octave - octave.round()).abs() < 1e-5);
    assert!((19.999..=4186.02).contains(&window.center_log2_hz.exp2()));
    // 1 sample into the captured onset, still before the first 256-sample hop.
    // Cubic onset bound for the fastest 2 ms attack: 3 * (1 / 96)^2 < 3.26e-4.
    assert!(window.amplitude < 3.26e-4);
    assert_eq!(p.mask[10], 1.0);
}

#[test]
fn silent_initial_expression_and_unmapped_notes_cannot_trigger() {
    let (mut p, mut c) = setup();
    select(&mut p, SpectralMotionShape::Sprinkle);
    c.events.extend([
        NoteEvent::PolyVolume {
            timing: 137,
            voice_id: VoiceID::ID(1),
            channel: Channel::Wildcard,
            key: Key::Wildcard,
            gain: 0.0,
        },
        note(137, 1),
    ]);
    process(&mut p, &mut c, 0, 256);
    assert_eq!(p.particles.engine.births, 0);
    p.tuning.mapped[60] = false;
    p.consume_note_event(note(0, 2));
    p.update_particle_sources();
    assert_eq!(p.particles.engine.births, 0);
}

#[test]
fn microtonal_sources_follow_pitch_without_rerolling_and_pins_are_independent() {
    let (mut p, _) = setup();
    select(&mut p, SpectralMotionShape::Sprinkle);
    p.configure_particles(120.0, true);
    p.tuning.active = true;
    p.tuning.frequencies[60] = 317.3;
    p.consume_note_event(note(0, 1));
    p.update_particle_sources();
    p.particles.engine.advance(500);
    let before = frame(&mut p);
    p.tuning.frequencies[60] = 391.71;
    p.update_particle_sources();
    let after = frame(&mut p);
    let a = before.windows.iter().find(|w| w.amplitude > 0.0).unwrap();
    let b = after.windows.iter().find(|w| w.amplitude > 0.0).unwrap();
    assert_eq!(a.amplitude, b.amplitude);
    assert!(((b.center_log2_hz - a.center_log2_hz) - (391.71_f32 / 317.3).log2()).abs() < 1e-5);
    p.params.pinned_notes.set(60, true);
    p.update_particle_sources();
    p.choke_voice(VoiceID::ID(1), Channel::Wildcard, Key::Wildcard);
    p.update_particle_sources();
    let births = p.particles.engine.births;
    for _ in 0..40 {
        p.particles.engine.advance(256);
        frame(&mut p);
    }
    assert!(p.particles.engine.births > births);
    p.params.pinned_notes.set(60, false);
    p.update_particle_sources();
    unsafe {
        p.params.motion_sync._internal_set_plain_value(true);
    }
    p.configure_particles(120.0, false);
    p.particles.engine.advance(961);
    frame(&mut p);
    assert_eq!(p.particles.engine.active_count(), 0);
}

#[test]
fn sustain_release_overlap_choke_and_slot_reuse_do_not_leak_identities() {
    let (mut p, mut c) = setup();
    select(&mut p, SpectralMotionShape::Sprinkle);
    c.events.extend([note(0, 1), note(0, 2)]);
    process(&mut p, &mut c, 0, 256);
    assert_eq!(p.particles.engine.births, 2);
    p.set_sustain(1, true);
    p.release_voice(VoiceID::ID(1), Channel::Wildcard, Key::Wildcard);
    p.update_particle_sources();
    assert!(voice(&p, 1).sustained);
    p.choke_voice(VoiceID::ID(2), Channel::Wildcard, Key::Wildcard);
    p.flush_terminated(&mut c, 0);
    let old = frame(&mut p);
    p.consume_note_event(note(0, 3));
    p.consume_note_event(tuning(0, 3, 12.0));
    p.update_particle_sources();
    let new = frame(&mut p);
    // Retiring windows retain their pitch even when their host slot is reused.
    assert_eq!(old.windows[1].center_log2_hz, new.windows[1].center_log2_hz);
    p.set_sustain(1, false);
    p.release_voice(VoiceID::ID(3), Channel::Wildcard, Key::Wildcard);
    p.update_particle_sources();
    let births = p.particles.engine.births;
    unsafe {
        p.params.motion_sync._internal_set_plain_value(true);
    }
    p.configure_particles(120.0, false);
    for _ in 0..500 {
        p.particles.engine.advance(256);
        frame(&mut p);
    }
    assert_eq!(p.particles.engine.births, births);
    assert_eq!(p.particles.engine.active_count(), 0);
}

#[test]
fn particle_audio_and_decisions_survive_host_partitions_and_automation() {
    fn render(
        block: usize,
        shape: SpectralMotionShape,
        direction: MotionDirection,
        smooth: bool,
        sync: bool,
    ) -> (Vec<f32>, u64, u64) {
        let (mut p, mut c) = setup();
        select(&mut p, shape);
        unsafe {
            p.params
                .motion_direction
                ._internal_set_plain_value(direction);
            p.params.smooth_spectral._internal_set_plain_value(smooth);
            p.params.motion_sync._internal_set_plain_value(sync);
            p.params
                .motion_rate_division
                ._internal_set_plain_value(MotionRateDivision::Sixteenth);
        }
        c.transport.playing = true;
        let events = [
            tuning(137, 1, 0.37),
            note(137, 1),
            note(1931, 2),
            tuning(8011, 1, -0.53),
            NoteEvent::NoteOff {
                timing: 9987,
                voice_id: VoiceID::ID(1),
                channel: Channel::Wildcard,
                key: Key::Wildcard,
                velocity: 0.0,
            },
        ];
        let changes = [1027, 7799, 14003, 17001];
        let mut result = Vec::new();
        let mut position = 0.0;
        let mut start = 0;
        while start < 24_576 {
            if changes.contains(&start) {
                unsafe {
                    p.params
                        .note_attack_ms
                        ._internal_set_plain_value(if start < 8000 { 25.0 } else { 0.0 });
                    p.params
                        .note_release_ms
                        ._internal_set_plain_value(if start < 8000 { 180.0 } else { 5000.0 });
                    p.params
                        .partials
                        ._internal_set_plain_value(if start < 8000 { 24 } else { 3 });
                    p.params
                        .harmonic_rolloff_db
                        ._internal_set_plain_value(if start < 8000 { 0.0 } else { 24.0 });
                    p.params
                        .motion_phase_percent
                        ._internal_set_plain_value(if start % 3 == 0 { 99.0 } else { 1.0 });
                    p.params
                        .motion_rate_hz
                        ._internal_set_plain_value(if start < 8000 { 17.0 } else { 7.0 });
                }
                c.transport.tempo = Some(if start < 8000 { 90.0 } else { 143.0 });
            }
            let len = block.min(24_576 - start).min(
                changes
                    .iter()
                    .copied()
                    .find(|x| *x > start)
                    .unwrap_or(24_576)
                    - start,
            );
            for event in events
                .iter()
                .filter(|e| (start..start + len).contains(&(e.timing() as usize)))
            {
                let mut event = *event;
                event.subtract_timing(start as u32);
                c.events.push_back(event);
            }
            c.transport.pos_beats = Some(position);
            result.extend(
                process_current(&mut p, &mut c, start, len)
                    .into_iter()
                    .next()
                    .unwrap(),
            );
            position += len as f64 / 48_000.0 * c.transport.tempo.unwrap() / 60.0;
            start += len;
        }
        (
            result,
            p.particles.engine.births,
            p.particles.engine.decision_hash,
        )
    }
    for (shape, direction) in [
        (SpectralMotionShape::Sprinkle, MotionDirection::Forward),
        (SpectralMotionShape::Cloud, MotionDirection::Forward),
        (SpectralMotionShape::Cloud, MotionDirection::Reverse),
        (SpectralMotionShape::Cloud, MotionDirection::Alternate),
    ] {
        for (smooth, sync) in [(false, false), (true, false), (false, true), (true, true)] {
            let (reference, births, hash) = render(1024, shape, direction, smooth, sync);
            assert!(births > 0);
            for block in [1, 64, 257] {
                let (audio, actual_births, actual_hash) =
                    render(block, shape, direction, smooth, sync);
                assert_eq!(
                    (actual_births, actual_hash),
                    (births, hash),
                    "{shape:?} block {block}"
                );
                let error = audio
                    .iter()
                    .zip(&reference)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f32, f32::max);
                assert!(error < 2e-5, "{shape:?} block {block}: {error}");
            }
        }
    }
}

#[test]
fn overflow_quality_switches_mode_automation_and_reset_do_not_allocate() {
    let (mut p, mut c) = setup();
    select(&mut p, SpectralMotionShape::Sprinkle);
    for id in 0..512 {
        c.events.push_back(note(13, id));
    }
    process(&mut p, &mut c, 0, 256);
    assert!(p.particles.engine.births <= MAX_PARTICLES as u64);
    assert!(p.particles.engine.dropped > 0);
    for (index, quality) in [
        FftQuality::Precise,
        FftQuality::Coarse,
        FftQuality::Balanced,
        FftQuality::Responsive,
        FftQuality::Rough,
    ]
    .into_iter()
    .enumerate()
    {
        unsafe {
            p.params.quality._internal_set_plain_value(quality);
        }
        process(&mut p, &mut c, (index + 1) * 256, 256);
    }
    for (index, shape) in [
        SpectralMotionShape::Cloud,
        SpectralMotionShape::Drift,
        SpectralMotionShape::Sprinkle,
    ]
    .into_iter()
    .enumerate()
    {
        select(&mut p, shape);
        process(&mut p, &mut c, (index + 6) * 256, 2048);
        assert!(p.mask.iter().all(|x| x.is_finite()));
    }
    nice_assert_no_alloc::assert_no_alloc(|| p.reset());
    assert_eq!(p.particles.engine.births, 0);
    assert_eq!(p.particles.engine.active_count(), 0);
}

#[test]
fn zero_depth_does_not_restart_pattern_and_manual_mutes_stay_closed() {
    let (mut p, mut c) = setup();
    select(&mut p, SpectralMotionShape::Cloud);
    p.params.motion_depth_db.smoothed.reset(0.0);
    unsafe {
        p.params.motion_depth_db._internal_set_plain_value(0.0);
    }
    for i in 0..20 {
        process(&mut p, &mut c, i * 256, 256);
    }
    assert!(p.particles.engine.births > 0);
    assert!(p.particles.gains[..513].iter().all(|x| *x == 1.0));
    select(&mut p, SpectralMotionShape::Cloud);
    for i in 0..MANUAL_MASK_POINTS {
        p.params.curve.set(i, MANUAL_CURVE_MUTE_DB);
    }
    process(&mut p, &mut c, 5120, 256);
    assert!(p.target_mask.iter().all(|x| *x == 0.0));
}

#[test]
fn shared_attack_release_shape_sprinkles_even_with_note_depth_zero() {
    fn render(
        shape: SpectralMotionShape,
        source: usize,
        attack: f32,
        release: f32,
    ) -> (Vec<f32>, u64) {
        let (mut p, mut c) = setup();
        select(&mut p, shape);
        unsafe {
            p.params.note_depth_db._internal_set_plain_value(0.0);
            // Disable ordinary note emphasis as well, to isolate particle timing.
            p.params.width_cents._internal_set_plain_value(1200.0);
            p.params.note_attack_ms._internal_set_plain_value(attack);
            p.params.note_release_ms._internal_set_plain_value(release);
        }
        p.params.note_depth_db.smoothed.reset(0.0);
        match source {
            1 => c.events.push_back(note(137, 1)),
            2 => p.params.pinned_notes.set(60, true),
            _ => {}
        }
        let mut output = Vec::new();
        let mut noise = 0x1234_5678_u32;
        for block in 0..64 {
            // Broadband input probes every free window; a single tone may miss them.
            let left: Vec<f32> = (0..256)
                .map(|_| {
                    noise ^= noise << 13;
                    noise ^= noise >> 17;
                    noise ^= noise << 5;
                    ((noise >> 8) as f32 / 16_777_216.0 - 0.5) * 0.2
                })
                .collect();
            let mut audio = [left.clone(), left];
            let mut buffer = Buffer::default();
            unsafe {
                buffer.set_slices(256, |slices| {
                    *slices = audio.iter_mut().map(|v| v.as_mut_slice()).collect();
                });
            }
            c.transport.pos_beats = Some(block as f64 * 256.0 / 24_000.0);
            nice_assert_no_alloc::assert_no_alloc(|| {
                p.process(
                    &mut buffer,
                    &mut AuxiliaryBuffers {
                        inputs: &mut [],
                        outputs: &mut [],
                    },
                    &mut c,
                );
            });
            output.extend(audio[0].iter().copied());
        }
        (output, p.particles.engine.births)
    }
    for source in 0..3 {
        let (short, births) = render(SpectralMotionShape::Sprinkle, source, 0.0, 70.0);
        assert!(births > 0);
        for (attack, release) in [(120.0, 70.0), (0.0, 400.0)] {
            let (changed, changed_births) =
                render(SpectralMotionShape::Sprinkle, source, attack, release);
            assert_eq!(births, changed_births);
            let difference = short
                .iter()
                .zip(&changed)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            assert!(
                difference > 1e-4,
                "source {source}, attack {attack}, release {release}"
            );
        }
        let (cloud_a, count_a) = render(SpectralMotionShape::Cloud, source, 0.0, 70.0);
        let (cloud_b, count_b) = render(SpectralMotionShape::Cloud, source, 120.0, 400.0);
        assert_eq!(count_a, count_b);
        let difference = cloud_a
            .iter()
            .zip(&cloud_b)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            difference == 0.0,
            "Cloud source {source}, peak difference {difference}"
        );
    }
}

#[test]
fn particles_run_without_notes_and_keep_depth_before_during_and_after_note_input() {
    for shape in [SpectralMotionShape::Sprinkle, SpectralMotionShape::Cloud] {
        for smooth in [false, true] {
            for pinned in [false, true] {
                let (mut p, mut c) = setup();
                select(&mut p, shape);
                unsafe {
                    p.params.note_depth_db._internal_set_plain_value(0.0);
                    p.params.smooth_spectral._internal_set_plain_value(smooth);
                    p.params
                        .motion_size_octaves
                        ._internal_set_plain_value(0.125);
                }
                p.params.note_depth_db.smoothed.reset(0.0);
                let expected = oiko_dsp::db_to_gain(-24.0);
                let mut start = 0;
                for stage in 0..3 {
                    if stage == 1 {
                        if pinned {
                            p.params.pinned_notes.set(60, true);
                        } else {
                            c.events.push_back(note(137, 1));
                        }
                    } else if stage == 2 {
                        if pinned {
                            p.params.pinned_notes.set(60, false);
                        } else {
                            c.events.push_back(NoteEvent::NoteOff {
                                timing: 137,
                                voice_id: VoiceID::ID(1),
                                channel: Channel::Wildcard,
                                key: Key::Wildcard,
                                velocity: 0.0,
                            });
                        }
                    }
                    let births = p.particles.engine.births;
                    let mut opened = false;
                    for block in 0..96 {
                        let audio = process(&mut p, &mut c, start, 1024);
                        start += 1024;
                        assert!(audio.iter().flatten().all(|s| s.is_finite()));
                        if block > 48 {
                            let gains = &p.particles.gains[..513];
                            assert!(gains.iter().all(|g| *g >= expected - 1e-5 && *g <= 1.0));
                            // Narrow windows leave background at the set Depth,
                            // including when free particles continue after release.
                            assert!(gains.iter().any(|g| (*g - expected).abs() < 1e-5));
                            opened |= gains.iter().any(|g| *g > expected + 1e-3);
                        }
                    }
                    assert!(p.particles.engine.births > births);
                    assert!(opened, "{shape:?}, stage {stage}");
                    assert_eq!(p.params.motion_depth_db.value(), 24.0);
                    assert_eq!(p.params.note_depth_db.value(), 0.0);
                }
            }
        }
    }
}

#[test]
fn legacy_shape_indices_recall_and_beta_automation_change_is_explicit() {
    use nice_plug::params::enums::Enum;
    let p = SpectralParams::default();
    assert_eq!(SpectralMotionShape::ids(), None);
    for (index, expected) in [
        SpectralMotionShape::Ripple,
        SpectralMotionShape::Harmonic,
        SpectralMotionShape::Drift,
        SpectralMotionShape::Scan,
        SpectralMotionShape::Notch,
        SpectralMotionShape::Saw,
        SpectralMotionShape::Sprinkle,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(SpectralMotionShape::from_index(index), expected);
        assert_eq!(p.motion_shape.preview_plain(index as f32 / 7.0), expected);
        assert_eq!(
            p.motion_shape.preview_normalized(expected),
            index as f32 / 7.0
        );
    }
    // Normalized 1.0 selects Cloud; persisted integer 6 still selects Sprinkle.
    assert_eq!(
        p.motion_shape.preview_plain(1.0),
        SpectralMotionShape::Cloud
    );
    assert_eq!(
        SpectralMotionShape::from_index(6),
        SpectralMotionShape::Sprinkle
    );
}

#[test]
fn sync_clock_stops_falls_back_to_last_tempo_and_seeks_without_catchup() {
    let (mut p, mut c) = setup();
    select(&mut p, SpectralMotionShape::Sprinkle);
    unsafe {
        p.params.motion_sync._internal_set_plain_value(true);
        p.params
            .motion_rate_division
            ._internal_set_plain_value(MotionRateDivision::Sixteenth);
    }
    c.transport.playing = false;
    c.transport.pos_beats = Some(12.0);
    c.transport.tempo = Some(143.0);
    c.events.push_back(note(137, 1));
    for start in (0..8192).step_by(256) {
        process_current(&mut p, &mut c, start, 256);
    }
    assert_eq!(p.particles.engine.births, 1);
    assert_eq!(p.particles.engine.active_count(), 0);
    c.transport.pos_beats = None;
    c.transport.tempo = None;
    for start in (8192..24576).step_by(256) {
        process_current(&mut p, &mut c, start, 256);
    }
    assert_eq!(p.particles.last_tempo, 143.0);
    assert!(p.particles.engine.births > 1);
    let before = p.particles.engine.births;
    c.transport.pos_beats = Some(1_000_000.0);
    process_current(&mut p, &mut c, 24576, 256);
    assert_eq!(p.particles.engine.births, before);
    for start in (24832..26112).step_by(256) {
        process_current(&mut p, &mut c, start, 256);
    }
    assert_eq!(p.particles.engine.active_count(), 0);
}

#[test]
fn particle_silence_dc_impulse_and_mono_stereo_are_finite_without_crossfeed() {
    fn render(
        shape: SpectralMotionShape,
        direction: MotionDirection,
        channels: usize,
        signal: usize,
    ) -> Vec<f32> {
        let (mut p, mut c) = setup();
        select(&mut p, shape);
        unsafe {
            p.params
                .motion_direction
                ._internal_set_plain_value(direction);
        }
        assert!(p.activate(
            &SpectralPlugin::AUDIO_IO_LAYOUTS[usize::from(channels == 1)],
            &BufferConfig {
                sample_rate: 48000.0,
                min_buffer_size: None,
                max_buffer_size: 1024,
                process_mode: ProcessMode::Offline,
            },
            &mut c
        ));
        p.reset();
        p.params.pinned_notes.set(60, true);
        let mut audio = vec![vec![0.0; 1024]; channels];
        let mut result = Vec::new();
        for start in (0..8192).step_by(1024) {
            for channel in &mut audio {
                for (i, value) in channel.iter_mut().enumerate() {
                    *value = match signal {
                        0 => 0.0,
                        1 => 0.1,
                        _ => {
                            if start + i == 137 {
                                0.1
                            } else {
                                0.0
                            }
                        }
                    };
                }
            }
            let mut buffer = Buffer::default();
            unsafe {
                buffer.set_slices(1024, |slices| {
                    *slices = audio.iter_mut().map(|v| v.as_mut_slice()).collect()
                });
            }
            c.transport.pos_beats = Some(start as f64 / 24000.0);
            nice_assert_no_alloc::assert_no_alloc(|| {
                p.process(
                    &mut buffer,
                    &mut AuxiliaryBuffers {
                        inputs: &mut [],
                        outputs: &mut [],
                    },
                    &mut c,
                )
            });
            assert!(
                audio
                    .iter()
                    .flatten()
                    .all(|x| x.is_finite() && x.abs() < 0.21)
            );
            if channels == 2 {
                assert_eq!(audio[0], audio[1]);
            }
            result.extend_from_slice(&audio[0]);
        }
        result
    }
    for (shape, direction) in [
        (SpectralMotionShape::Sprinkle, MotionDirection::Forward),
        (SpectralMotionShape::Cloud, MotionDirection::Forward),
        (SpectralMotionShape::Cloud, MotionDirection::Reverse),
        (SpectralMotionShape::Cloud, MotionDirection::Alternate),
    ] {
        for signal in 0..3 {
            let mono = render(shape, direction, 1, signal);
            let stereo = render(shape, direction, 2, signal);
            assert_eq!(mono, stereo);
            if signal == 0 {
                assert!(mono.iter().all(|x| *x == 0.0));
            }
        }
    }
}

#[test]
// Use release for callback timing and debug for allocation assertions. NicePlug
// and nice-assert-no-alloc both disable allocation checking in release builds.
#[ignore = "manual callback matrix: release timing / debug allocation checks; prints CSV"]
fn benchmark_particle_processor() {
    use std::time::{Duration, Instant};
    println!(
        "sample_rate,fft,channels,shape,direction,max_active,callback_samples,worst_us,deadline_us"
    );
    for sample_rate in [44100.0, 48000.0, 96000.0, 192000.0, 384000.0] {
        for quality in [
            FftQuality::Rough,
            FftQuality::Coarse,
            FftQuality::Responsive,
            FftQuality::Balanced,
            FftQuality::Precise,
        ] {
            for channels in [1, 2] {
                for callback_samples in [64, 256, 1024] {
                    for (shape, direction) in [
                        (SpectralMotionShape::Drift, MotionDirection::Forward),
                        (SpectralMotionShape::Sprinkle, MotionDirection::Forward),
                        (SpectralMotionShape::Cloud, MotionDirection::Forward),
                        (SpectralMotionShape::Cloud, MotionDirection::Reverse),
                        (SpectralMotionShape::Cloud, MotionDirection::Alternate),
                    ] {
                        let (mut p, mut c) = setup();
                        select(&mut p, shape);
                        unsafe {
                            p.params
                                .motion_direction
                                ._internal_set_plain_value(direction);
                            p.params.quality._internal_set_plain_value(quality);
                            p.params.motion_size_octaves._internal_set_plain_value(8.0);
                            p.params.motion_rate_hz._internal_set_plain_value(32.0);
                            p.params.partials._internal_set_plain_value(24);
                            p.params.harmonic_rolloff_db._internal_set_plain_value(0.0);
                            p.params.note_attack_ms._internal_set_plain_value(0.0);
                            p.params.note_release_ms._internal_set_plain_value(5000.0);
                        }
                        p.activate(
                            &SpectralPlugin::AUDIO_IO_LAYOUTS[usize::from(channels == 1)],
                            &BufferConfig {
                                sample_rate,
                                min_buffer_size: None,
                                max_buffer_size: callback_samples as u32,
                                process_mode: ProcessMode::Offline,
                            },
                            &mut c,
                        );
                        p.reset();
                        for id in 0..64 {
                            c.events.push_back(NoteEvent::NoteOn {
                                timing: 0,
                                voice_id: VoiceID::ID(id),
                                channel: Channel::Number(1),
                                key: Key::Number(36 + id as u8),
                                velocity: 1.0,
                            });
                        }
                        let mut audio = vec![vec![0.1; callback_samples]; channels];
                        let mut buffer = Buffer::default();
                        unsafe {
                            buffer.set_slices(callback_samples, |slices| {
                                *slices = audio.iter_mut().map(|v| v.as_mut_slice()).collect()
                            });
                        }
                        let mut worst = Duration::ZERO;
                        let mut active = 0;
                        // At least 64 FFT hops, including full initial Sprinkle pool.
                        for block in 0..quality.size() * 16 / callback_samples {
                            c.transport.pos_beats = Some(
                                block as f64 * callback_samples as f64 / sample_rate as f64 * 2.0,
                            );
                            // Fresh input avoids benchmarking recirculated output or decayed silence.
                            for channel in buffer.iter_samples() {
                                for sample in channel {
                                    *sample = 0.1;
                                }
                            }
                            let began = Instant::now();
                            nice_assert_no_alloc::assert_no_alloc(|| {
                                p.process(
                                    &mut buffer,
                                    &mut AuxiliaryBuffers {
                                        inputs: &mut [],
                                        outputs: &mut [],
                                    },
                                    &mut c,
                                )
                            });
                            worst = worst.max(began.elapsed());
                            active = active.max(p.particles.engine.active_count());
                        }
                        println!(
                            "{sample_rate},{},{channels},{shape:?},{direction:?},{active},{callback_samples},{:.3},{:.3}",
                            quality.size(),
                            worst.as_secs_f64() * 1e6,
                            callback_samples as f64 / sample_rate as f64 * 1e6
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn original_shape_values_keep_integer_identity_and_record_vst3_remapping() {
    use SpectralMotionShape::*;
    use nice_plug::params::enums::Enum;
    let p = SpectralParams::default();
    // Persisted and CLAP values are integer indices and keep their meaning.
    // VST3 normalizes by the step count, so values saved as index/6 map to the
    // listed shapes. These are encoding expectations, not fixtures captured
    // from a native host.
    let cases = [
        (Ripple, 0, 0.0, Ripple),
        (Harmonic, 1, 1.0 / 6.0, Harmonic),
        (Drift, 2, 2.0 / 6.0, Drift),
        (Scan, 3, 0.5, Notch),
        (Notch, 4, 4.0 / 6.0, Saw),
        (Saw, 5, 5.0 / 6.0, Sprinkle),
        (Sprinkle, 6, 1.0, Cloud), // Stored index 6 stays Sprinkle; VST3 value 1.0 is Cloud.
    ];
    for (expected, old_index, old_vst_value, remapped) in cases {
        assert_eq!(SpectralMotionShape::from_index(old_index), expected);
        assert_eq!(expected.to_index(), old_index);
        assert_eq!(
            p.motion_shape
                .preview_plain(old_index as f32 / p.motion_shape.step_count().unwrap() as f32),
            expected
        );
        assert_eq!(p.motion_shape.preview_plain(old_vst_value), remapped);
    }
}

#[test]
fn original_looping_shapes_keep_bit_identical_masks() {
    for shape in [
        SpectralMotionShape::Ripple,
        SpectralMotionShape::Harmonic,
        SpectralMotionShape::Drift,
        SpectralMotionShape::Scan,
        SpectralMotionShape::Notch,
        SpectralMotionShape::Saw,
    ] {
        let (mut p, mut c) = setup();
        select(&mut p, shape);
        c.events.push_back(note(0, 1));
        process(&mut p, &mut c, 0, 1024);
        let mut reference_left = vec![0.0; 513];
        let mut reference_right = vec![0.0; 513];
        let config = MaskConfig {
            sample_rate: p.sample_rate,
            fft_size: 1024,
            note_depth_db: p.params.note_depth_db.value(),
            note_layer_enabled: true,
            width_cents: p.params.width_cents.value(),
            partials: p.params.partials.value() as usize,
            harmonic_rolloff_db_per_octave: p.params.harmonic_rolloff_db.value(),
            motion: MotionConfig {
                shape: shape.into(),
                depth_db: 24.0,
                phase: p.analysis_display.motion_phase(),
                size_octaves: p.params.motion_size_octaves.value(),
            },
        };
        spectral_dsp::build_stereo_masks_with_voices_precomputed(
            &mut reference_left,
            &mut reference_right,
            &p.manual_curve_cache,
            &p.mask_voices,
            config,
            &p.plans.as_ref().unwrap()[0].prepared.mask_workspace,
        );
        assert_eq!(p.target_mask, reference_left, "{shape:?}");
        assert_eq!(p.target_mask_right, reference_right, "{shape:?}");
    }
}
