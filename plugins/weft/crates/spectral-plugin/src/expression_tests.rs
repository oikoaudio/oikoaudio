use super::*;
use crate::parameters::FftQuality;
use nice_plug::params::InternalParamMut;
use std::collections::VecDeque;

struct Context {
    transport: Transport,
    events: VecDeque<NoteEvent<()>>,
    output: Vec<NoteEvent<()>>,
}
impl Context {
    fn new() -> Self {
        let mut transport = Transport::new(48_000.0);
        transport.tempo = Some(120.0);
        transport.playing = true;
        Self {
            transport,
            events: VecDeque::new(),
            output: Vec::with_capacity(1024),
        }
    }
}
impl ActivateContext<SpectralPlugin> for Context {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute(&self, _: ()) {}
    fn set_latency_samples(&self, _: u32) {}
    fn set_current_voice_capacity(&self, _: u32) {}
}
impl ProcessContext<SpectralPlugin> for Context {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute_background(&self, _: ()) {}
    fn execute_gui(&self, _: ()) {}
    fn transport(&self) -> &Transport {
        &self.transport
    }
    fn next_event(&mut self) -> Option<NoteEvent<()>> {
        self.events.pop_front()
    }
    fn send_event(&mut self, event: NoteEvent<()>) {
        self.output.push(event);
    }
    fn set_latency_samples(&self, _: u32) {}
    fn set_current_voice_capacity(&self, _: u32) {}
}
fn setup() -> (SpectralPlugin, Context) {
    let mut plugin = SpectralPlugin::default();
    let mut context = Context::new();
    unsafe {
        plugin
            .params
            .quality
            ._internal_set_plain_value(FftQuality::Rough);
    }
    assert!(plugin.activate(
        &SpectralPlugin::AUDIO_IO_LAYOUTS[0],
        &BufferConfig {
            sample_rate: 48_000.0,
            min_buffer_size: None,
            max_buffer_size: 2048,
            process_mode: ProcessMode::Realtime,
        },
        &mut context
    ));
    plugin.reset();
    (plugin, context)
}
fn process(
    plugin: &mut SpectralPlugin,
    context: &mut Context,
    start: usize,
    samples: usize,
) -> [Vec<f32>; 2] {
    context.transport.pos_beats = Some(start as f64 / 24_000.0);
    process_current(plugin, context, start, samples)
}
fn process_current(
    plugin: &mut SpectralPlugin,
    context: &mut Context,
    start: usize,
    samples: usize,
) -> [Vec<f32>; 2] {
    let mut audio = std::array::from_fn(|_| {
        (start..start + samples)
            .map(|sample| (sample as f32 * 0.061).sin() * 0.1)
            .collect::<Vec<_>>()
    });
    let mut buffer = Buffer::default();
    unsafe {
        buffer.set_slices(samples, |slices| {
            *slices = audio.iter_mut().map(|v| v.as_mut_slice()).collect()
        });
    }
    nice_assert_no_alloc::assert_no_alloc(|| {
        plugin.process(
            &mut buffer,
            &mut AuxiliaryBuffers {
                inputs: &mut [],
                outputs: &mut [],
            },
            context,
        );
    });
    audio
}
fn note(timing: u32, id: i32) -> NoteEvent<()> {
    NoteEvent::NoteOn {
        timing,
        voice_id: Some(id),
        channel: 1,
        note: 60,
        velocity: 1.0,
    }
}
fn tuning(timing: u32, id: i32, value: f32) -> NoteEvent<()> {
    NoteEvent::PolyTuning {
        timing,
        voice_id: Some(id),
        channel: 255,
        note: 255,
        tuning: value,
    }
}
fn voice(plugin: &SpectralPlugin, id: i32) -> &VoiceState {
    plugin
        .voices
        .iter()
        .find(|v| v.occupied && v.voice_id == Some(id))
        .unwrap()
}

#[test]
fn late_note_only_affects_the_frame_after_its_offset() {
    let (mut p, mut c) = setup();
    c.events.push_back(note(1023, 1));
    process(&mut p, &mut c, 0, 1024);
    assert_eq!(
        p.note_level_history
            .iter()
            .filter(|frame| frame[60] > 0.0)
            .count(),
        1
    );
    // Only one sample of attack has elapsed, not a full hop.
    let expected = 1.0 - (-1.0 / (48.0 * p.params.note_attack_ms.value())).exp();
    assert!((voice(&p, 1).level - expected).abs() < 1e-6);
}

#[test]
fn expressions_change_live_voices_and_keep_same_key_ids_independent() {
    let (mut p, mut c) = setup();
    c.events.extend([note(0, 10), note(0, 11)]);
    process(&mut p, &mut c, 0, 256);
    c.events.extend([
        tuning(32, 10, 1.25),
        NoteEvent::PolyVolume {
            timing: 32,
            voice_id: Some(10),
            channel: 255,
            note: 255,
            gain: 2.0,
        },
        NoteEvent::PolyPan {
            timing: 32,
            voice_id: Some(10),
            channel: 255,
            note: 255,
            pan: -1.0,
        },
        NoteEvent::PolyBrightness {
            timing: 32,
            voice_id: Some(10),
            channel: 255,
            note: 255,
            brightness: 0.8,
        },
        NoteEvent::PolyPressure {
            timing: 32,
            voice_id: Some(10),
            channel: 255,
            note: 255,
            pressure: 0.3,
        },
    ]);
    process(&mut p, &mut c, 256, 256);
    let a = voice(&p, 10).expression;
    let b = voice(&p, 11).expression;
    assert_eq!(
        (a.tuning_semitones, a.gain, a.pan, a.timbre, a.pressure),
        (1.25, 2.0, -1.0, 0.8, 0.3)
    );
    assert_eq!(
        (b.tuning_semitones, b.gain, b.pan, b.timbre, b.pressure),
        (0.0, 1.0, 0.0, 0.5, 1.0)
    );
    c.events.push_back(NoteEvent::NoteOff {
        timing: 0,
        voice_id: Some(10),
        channel: 255,
        note: 255,
        velocity: 0.0,
    });
    process(&mut p, &mut c, 512, 256);
    assert!(!voice(&p, 10).held);
    assert!(voice(&p, 11).held);
}

#[test]
fn same_sample_initial_expression_can_precede_note_on_but_does_not_leak() {
    let (mut p, mut c) = setup();
    c.events.extend([
        tuning(23, 1, 0.75),
        note(23, 1),
        tuning(24, 2, 8.0),
        note(25, 2),
    ]);
    process(&mut p, &mut c, 0, 256);
    assert_eq!(voice(&p, 1).expression.tuning_semitones, 0.75);
    assert_eq!(voice(&p, 2).expression.tuning_semitones, 0.0);
}

#[test]
fn wildcard_release_choke_zero_velocity_and_voice_end() {
    let (mut p, mut c) = setup();
    let mut zero = note(0, 12);
    if let NoteEvent::NoteOn { velocity, .. } = &mut zero {
        *velocity = 0.0;
    }
    c.events.extend([note(0, 10), note(0, 11), zero]);
    process(&mut p, &mut c, 0, 256);
    assert!(voice(&p, 12).held);
    c.events.push_back(NoteEvent::NoteOff {
        timing: 0,
        voice_id: None,
        channel: 1,
        note: 60,
        velocity: 0.0,
    });
    process(&mut p, &mut c, 256, 256);
    assert!(p.voices.iter().all(|v| !v.held));
    c.events.push_back(NoteEvent::Choke {
        timing: 19,
        voice_id: None,
        channel: 255,
        note: 255,
    });
    process(&mut p, &mut c, 512, 256);
    assert!(p.voices.iter().all(|v| !v.occupied));
    assert_eq!(c.output.len(), 3);
    assert!(
        c.output
            .iter()
            .all(|e| matches!(e, NoteEvent::VoiceTerminated { timing: 19, .. }))
    );
}

#[test]
fn native_values_override_midi_y_z_and_expression_is_not_brightness() {
    let (mut p, mut c) = setup();
    c.events.extend([
        note(0, 1),
        tuning(0, 1, 0.25),
        NoteEvent::PolyBrightness {
            timing: 0,
            voice_id: Some(1),
            channel: 1,
            note: 60,
            brightness: 0.8,
        },
        NoteEvent::PolyPressure {
            timing: 0,
            voice_id: Some(1),
            channel: 1,
            note: 60,
            pressure: 0.4,
        },
        NoteEvent::MidiCC {
            timing: 1,
            channel: 1,
            cc: 74,
            value: 0.1,
        },
        NoteEvent::MidiChannelPressure {
            timing: 1,
            channel: 1,
            pressure: 0.1,
        },
        NoteEvent::MidiPitchBend {
            timing: 1,
            channel: 1,
            value: 1.0,
        },
        NoteEvent::PolyExpression {
            timing: 2,
            voice_id: Some(1),
            channel: 1,
            note: 60,
            expression: 0.5,
        },
    ]);
    process(&mut p, &mut c, 0, 256);
    let expression = voice(&p, 1).expression;
    assert_eq!(
        (
            expression.tuning_semitones,
            expression.gain,
            expression.timbre,
            expression.pressure
        ),
        (48.25, 0.5, 0.8, 0.4)
    );
    c.events.push_back(note(0, 2));
    process(&mut p, &mut c, 256, 256);
    assert_eq!(voice(&p, 2).expression.timbre, 0.1);
}

#[test]
fn sample_timing_and_audio_are_independent_of_host_partition() {
    fn render(block: usize) -> (Vec<f32>, f32) {
        let (mut p, mut c) = setup();
        let events = [
            note(137, 1),
            tuning(777, 1, 7.0),
            NoteEvent::PolyPan {
                timing: 931,
                voice_id: Some(1),
                channel: 1,
                note: 60,
                pan: 1.0,
            },
            NoteEvent::NoteOff {
                timing: 1987,
                voice_id: Some(1),
                channel: 1,
                note: 60,
                velocity: 0.0,
            },
        ];
        let mut result = Vec::new();
        for start in (0..4096).step_by(block) {
            let len = block.min(4096 - start);
            for event in events
                .iter()
                .filter(|e| (start..start + len).contains(&(e.timing() as usize)))
            {
                let mut event = *event;
                event.subtract_timing(start as u32);
                c.events.push_back(event);
            }
            result.extend(
                process(&mut p, &mut c, start, len)
                    .into_iter()
                    .next()
                    .unwrap(),
            );
        }
        (result, p.analysis_display.motion_phase())
    }
    let (reference, phase) = render(1024);
    for block in [1, 64, 257] {
        let (audio, actual_phase) = render(block);
        let error = audio
            .iter()
            .zip(&reference)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(error < 2e-5, "block {block}: audio error {error}");
        assert!((actual_phase - phase).abs() < 1e-7);
    }
}

#[test]
fn automation_smoother_advances_only_elapsed_samples() {
    let (mut p, mut c) = setup();
    p.params.output_gain_db.smoothed.reset(0.0);
    process(&mut p, &mut c, 0, 255);
    unsafe {
        p.params.output_gain_db._internal_set_plain_value(-12.0);
    }
    p.params.output_gain_db.smoothed.set_target(48_000.0, -12.0);
    process(&mut p, &mut c, 255, 1);
    assert!((p.params.output_gain_db.smoothed.previous_value() + 0.0125).abs() < 1e-5);
}

#[test]
fn stealing_and_release_send_voice_end_without_allocating() {
    let (mut p, mut c) = setup();
    for id in 0..129 {
        c.events.push_back(note(0, id));
    }
    process(&mut p, &mut c, 0, 256);
    assert_eq!(c.output.len(), 1);
    unsafe {
        p.params.note_release_ms._internal_set_plain_value(0.0);
    }
    c.events.push_back(NoteEvent::NoteOff {
        timing: 3,
        voice_id: None,
        channel: 255,
        note: 255,
        velocity: 0.0,
    });
    process(&mut p, &mut c, 256, 256);
    assert_eq!(c.output.len(), 129);
    assert!(p.voices.iter().all(|v| !v.occupied));
}

fn cc(p: &mut SpectralPlugin, channel: u8, control: u8, value: u8) {
    p.consume_note_event(NoteEvent::MidiCC {
        timing: 0,
        channel,
        cc: control,
        value: value as f32 / 127.0,
    });
}
fn rpn(p: &mut SpectralPlugin, channel: u8, parameter: u8, value: u8) {
    cc(p, channel, 101, 0);
    cc(p, channel, 100, parameter);
    cc(p, channel, 6, value);
}
#[test]
fn mpe_zones_combine_master_and_member_controls_and_rpn_bend_ranges() {
    let (mut p, mut c) = setup();
    rpn(&mut p, 0, 6, 3);
    rpn(&mut p, 15, 6, 2);
    rpn(&mut p, 1, 0, 12);
    cc(&mut p, 1, 38, 50);
    p.consume_note_event(note(0, 1));
    p.consume_note_event(NoteEvent::NoteOn {
        timing: 0,
        voice_id: Some(2),
        channel: 14,
        note: 60,
        velocity: 1.0,
    });
    for (channel, value) in [(0, 1.0), (1, 1.0), (15, 0.0), (14, 1.0)] {
        p.consume_note_event(NoteEvent::MidiPitchBend {
            timing: 0,
            channel,
            value,
        });
    }
    p.consume_note_event(NoteEvent::MidiChannelPressure {
        timing: 0,
        channel: 0,
        pressure: 0.5,
    });
    p.consume_note_event(NoteEvent::MidiChannelPressure {
        timing: 0,
        channel: 1,
        pressure: 0.6,
    });
    p.consume_note_event(NoteEvent::MidiCC {
        timing: 0,
        channel: 0,
        cc: 74,
        value: 0.7,
    });
    p.consume_note_event(NoteEvent::MidiCC {
        timing: 0,
        channel: 1,
        cc: 74,
        value: 0.6,
    });
    process(&mut p, &mut c, 0, 256);
    assert_eq!(voice(&p, 1).expression.tuning_semitones, 14.5);
    assert_eq!(voice(&p, 2).expression.tuning_semitones, 46.0);
    assert!((voice(&p, 1).expression.pressure - 0.3).abs() < 1e-6);
    assert!((voice(&p, 1).expression.timbre - 0.8).abs() < 1e-6);
    cc(&mut p, 0, 64, 127);
    p.release_voice(Some(1), 1, 60);
    assert!(voice(&p, 1).sustained);
    cc(&mut p, 0, 64, 0);
    assert!(!voice(&p, 1).sustained);
    rpn(&mut p, 0, 6, 14);
    assert_eq!(p.midi_expression.master_for(14), Some(0));
    assert_eq!(p.midi_expression.master_for(15), None);
}

#[test]
fn invalid_expression_values_do_not_poison_audio_or_voice_state() {
    let (mut p, mut c) = setup();
    c.events.extend([
        note(0, 1),
        tuning(1, 1, f32::NAN),
        NoteEvent::PolyVolume {
            timing: 1,
            voice_id: Some(1),
            channel: 1,
            note: 60,
            gain: f32::INFINITY,
        },
        NoteEvent::MidiPitchBend {
            timing: 1,
            channel: 255,
            value: 1.0,
        },
    ]);
    let audio = process(&mut p, &mut c, 0, 256);
    assert!(audio.iter().flatten().all(|v| v.is_finite()));
    assert_eq!(voice(&p, 1).expression.tuning_semitones, 0.0);
    assert_eq!(voice(&p, 1).expression.gain, 1.0);
}

#[test]
fn same_sample_wildcards_apply_to_new_voices_in_order_with_specific_values() {
    let (mut p, mut c) = setup();
    c.events.extend([
        NoteEvent::PolyVolume {
            timing: 0,
            voice_id: None,
            channel: 255,
            note: 255,
            gain: 0.5,
        },
        note(0, 1),
        note(0, 2),
        NoteEvent::PolyVolume {
            timing: 0,
            voice_id: Some(1),
            channel: 255,
            note: 255,
            gain: 2.0,
        },
        NoteEvent::PolyVolume {
            timing: 0,
            voice_id: None,
            channel: 255,
            note: 255,
            gain: 0.75,
        },
        NoteEvent::PolyVolume {
            timing: 0,
            voice_id: Some(2),
            channel: 255,
            note: 255,
            gain: 3.0,
        },
    ]);
    process(&mut p, &mut c, 0, 256);
    assert_eq!(voice(&p, 1).expression.gain, 0.75);
    assert_eq!(voice(&p, 2).expression.gain, 3.0);
}

#[test]
fn mpe_configuration_resets_changed_channels_and_member_ranges_are_shared() {
    let (mut p, mut c) = setup();
    c.events.push_back(note(0, 1));
    process(&mut p, &mut c, 0, 256);
    rpn(&mut p, 0, 6, 3);
    assert!(p.voices.iter().all(|voice| !voice.occupied));
    assert_eq!(p.terminated_count, 1);
    p.flush_terminated(&mut c, 0);
    rpn(&mut p, 2, 0, 7);
    p.midi_expression.pitch_bend(1, 1.0);
    p.midi_expression.pitch_bend(3, 1.0);
    assert_eq!(p.midi_expression.bend(1, 48.0), 7.0);
    assert_eq!(p.midi_expression.bend(3, 48.0), 7.0);
    rpn(&mut p, 15, 6, 15);
    rpn(&mut p, 0, 6, 0);
    assert_eq!(p.midi_expression.master_for(0), Some(15));
}

#[path = "particle_tests.rs"]
mod particle_tests;
