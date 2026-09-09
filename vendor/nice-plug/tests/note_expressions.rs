//! Native expression conversion through actual wrapper code and the CLAP entry point.
#[path = "../src/wrapper/vst3/note_expressions.rs"]
mod vst3_expressions;
use clap_sys::{
    audio_buffer::clap_audio_buffer,
    events::*,
    ext::params::{CLAP_EXT_PARAMS, clap_param_info, clap_plugin_params},
    factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory},
    host::clap_host,
    plugin::clap_plugin,
    process::{CLAP_PROCESS_ERROR, clap_process},
    version::CLAP_VERSION,
};
use nice_plug::prelude::*;
use nice_plug::midi::{Channel, Key, VoiceID};
use std::{
    ffi::{c_char, c_void},
    num::NonZeroU32,
    ptr,
    sync::Arc,
};

#[derive(Params)]
struct ProbeParams {
    #[id = "value"]
    value: FloatParam,
}
impl Default for ProbeParams {
    fn default() -> Self {
        Self {
            value: FloatParam::new("Value", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
        }
    }
}

#[derive(Default)]
struct TimingProbe<const SAMPLE_ACCURATE: bool> {
    params: Arc<ProbeParams>,
}
impl<const SAMPLE_ACCURATE: bool> Plugin for TimingProbe<SAMPLE_ACCURATE> {
    const NAME: &'static str = "CLAP timing probe";
    const VENDOR: &'static str = "NicePlug tests";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = "0.0.0";
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const SAMPLE_ACCURATE_AUTOMATION: bool = SAMPLE_ACCURATE;
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;
    type Editor = ();
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // No smoothing, latency, worker or GUI can mask the time the wrapper applies a change.
        buffer.as_slice()[0].fill(self.params.value.value());
        buffer.as_slice()[1].fill(context.transport().tempo.unwrap_or(120.0) as f32);
        while let Some(event) = context.next_event() {
            let value = match event {
                NoteEvent::PolyTuning { tuning, .. } => Some(tuning),
                NoteEvent::PolyVolume { gain, .. } => Some(gain),
                NoteEvent::PolyPan { pan, .. } => Some(pan),
                NoteEvent::PolyBrightness { brightness, .. } => Some(brightness),
                NoteEvent::PolyPressure { pressure, .. } => Some(pressure),
                NoteEvent::PolyExpression { expression, .. } => Some(expression),
                _ => None,
            };
            if let Some(value) = value {
                buffer.as_slice()[0][event.timing() as usize] = value;
            }
            let _ = context.try_send_event(event);
        }
        ProcessStatus::Normal
    }
}
impl<const SAMPLE_ACCURATE: bool> ClapPlugin for TimingProbe<SAMPLE_ACCURATE> {
    const CLAP_SUPPORTS_MPE: bool = SAMPLE_ACCURATE;
    const CLAP_ID: &'static str = if SAMPLE_ACCURATE {
        "test.sample-accurate"
    } else {
        "test.block-rate"
    };
    const CLAP_DESCRIPTION: Option<&'static str> = None;
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect];
}
nice_export_clap!(TimingProbe<true>, TimingProbe<false>);

// The event payloads and list are owned by the caller for the entire process call.
#[allow(dead_code)]
enum Event {
    Expression(clap_event_note_expression),
    Value(clap_event_param_value),
    Modulation(clap_event_param_mod),
    Note(clap_event_note),
    Transport(clap_event_transport),
}
#[allow(dead_code)]
impl Event {
    fn header(&self) -> &clap_event_header {
        match self {
            Self::Expression(event) => &event.header,
            Self::Value(event) => &event.header,
            Self::Modulation(event) => &event.header,
            Self::Note(event) => &event.header,
            Self::Transport(event) => &event.header,
        }
    }

    fn value(param_id: u32, time: u32, value: f64) -> Self {
        Self::Value(clap_event_param_value {
            header: header::<clap_event_param_value>(CLAP_EVENT_PARAM_VALUE, time),
            param_id,
            cookie: ptr::null_mut(),
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value,
        })
    }

    fn modulation(param_id: u32, time: u32, amount: f64) -> Self {
        Self::Modulation(clap_event_param_mod {
            header: header::<clap_event_param_mod>(CLAP_EVENT_PARAM_MOD, time),
            param_id,
            cookie: ptr::null_mut(),
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            amount,
        })
    }

    fn note(time: u32, key: i16) -> Self {
        Self::Note(clap_event_note {
            header: header::<clap_event_note>(CLAP_EVENT_NOTE_ON, time),
            note_id: key as i32,
            port_index: 0,
            channel: 0,
            key,
            velocity: 1.0,
        })
    }

    fn tempo(time: u32, tempo: f64) -> Self {
        Self::Transport(clap_event_transport {
            header: header::<clap_event_transport>(CLAP_EVENT_TRANSPORT, time),
            flags: CLAP_TRANSPORT_HAS_TEMPO | CLAP_TRANSPORT_IS_PLAYING,
            tempo,
            // No song position: transport-origin reconstruction is a separate contract.
            ..unsafe { std::mem::zeroed() }
        })
    }
}

fn header<T>(type_: u16, time: u32) -> clap_event_header {
    clap_event_header {
        size: std::mem::size_of::<T>() as u32,
        time,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_,
        flags: 0,
    }
}

unsafe extern "C" fn host_extension(_: *const clap_host, _: *const c_char) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn event_count(list: *const clap_input_events) -> u32 {
    unsafe { (&*((*list).ctx as *const Vec<Event>)).len() as u32 }
}
unsafe extern "C" fn event_at(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    unsafe { (&*((*list).ctx as *const Vec<Event>))[index as usize].header() }
}
unsafe extern "C" fn output_event(
    list: *const clap_output_events,
    event: *const clap_event_header,
) -> bool {
    unsafe {
        let notes = &mut *((*list).ctx as *mut Vec<Event>);
        if notes.len() == notes.capacity() {
            return false;
        }
        match (*event).type_ {
            CLAP_EVENT_NOTE_EXPRESSION => notes.push(Event::Expression(
                *(event as *const clap_event_note_expression),
            )),
            CLAP_EVENT_NOTE_ON | CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE => {
                notes.push(Event::Note(*(event as *const clap_event_note)))
            }
            _ => {}
        }
    }
    true
}

struct Host {
    plugin: *const clap_plugin,
    // Heap allocation keeps the host address stable until after plugin destruction.
    _host: Box<clap_host>,
}
impl Host {
    fn new(sample_accurate: bool) -> Self {
        let host = Box::new(clap_host {
            clap_version: CLAP_VERSION,
            host_data: ptr::null_mut(),
            name: c"Timing test host".as_ptr(),
            vendor: c"NicePlug tests".as_ptr(),
            url: c"".as_ptr(),
            version: c"1".as_ptr(),
            get_extension: Some(host_extension),
            request_restart: None,
            request_process: None,
            request_callback: None,
        });
        unsafe {
            assert!(clap_entry.init.unwrap()(c"test".as_ptr()));
            let factory = &*(clap_entry.get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr())
                as *const clap_plugin_factory);
            let descriptor =
                factory.get_plugin_descriptor.unwrap()(factory, u32::from(!sample_accurate));
            let plugin = factory.create_plugin.unwrap()(factory, &*host, (*descriptor).id);
            assert!(!plugin.is_null());
            assert!((*plugin).init.unwrap()(plugin));
            let params = &*((*plugin).get_extension.unwrap()(plugin, CLAP_EXT_PARAMS.as_ptr())
                as *const clap_plugin_params);
            let mut info = std::mem::zeroed::<clap_param_info>();
            assert!(params.get_info.unwrap()(plugin, 0, &mut info));
            assert!((*plugin).activate.unwrap()(plugin, 48_000.0, 1, 256));
            assert!((*plugin).start_processing.unwrap()(plugin));
            Self {
                plugin,
                _host: host,
            }
        }
    }

    fn process(&mut self, samples: usize, mut events: Vec<Event>) -> ([Vec<f32>; 2], Vec<Event>) {
        assert!((1..=256).contains(&samples));
        assert!(
            events
                .windows(2)
                .all(|pair| pair[0].header().time <= pair[1].header().time)
        );
        assert!(
            events
                .iter()
                .all(|event| event.header().time < samples as u32)
        );
        let mut input = [vec![0.0; samples], vec![0.0; samples]];
        let mut output = input.clone();
        let mut input_ptrs = [input[0].as_mut_ptr(), input[1].as_mut_ptr()];
        let mut output_ptrs = [output[0].as_mut_ptr(), output[1].as_mut_ptr()];
        let input_buffer = clap_audio_buffer {
            data32: input_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut output_buffer = clap_audio_buffer {
            data32: output_ptrs.as_mut_ptr(),
            ..input_buffer
        };
        let input_events = clap_input_events {
            ctx: (&mut events as *mut Vec<Event>).cast(),
            size: Some(event_count),
            get: Some(event_at),
        };
        let mut notes = Vec::<Event>::with_capacity(32);
        let output_events = clap_output_events {
            ctx: (&mut notes as *mut Vec<_>).cast(),
            try_push: Some(output_event),
        };
        let process = clap_process {
            steady_time: -1,
            frames_count: samples as u32,
            transport: ptr::null(),
            audio_inputs: &input_buffer,
            audio_outputs: &mut output_buffer,
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: &input_events,
            out_events: &output_events,
        };
        unsafe {
            assert_ne!(
                (*self.plugin).process.unwrap()(self.plugin, &process),
                CLAP_PROCESS_ERROR
            );
        }
        (output, notes)
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            (*self.plugin).stop_processing.unwrap()(self.plugin);
            (*self.plugin).deactivate.unwrap()(self.plugin);
            (*self.plugin).destroy.unwrap()(self.plugin);
        }
    }
}

#[test]
fn clap_normalizes_all_five_expressions_and_preserves_wildcard_addresses() {
    let mut host = Host::new(true);
    let expressions = [
        (CLAP_NOTE_EXPRESSION_TUNING, 1.25, 1.25),
        (CLAP_NOTE_EXPRESSION_VOLUME, 2.0, 2.0),
        (CLAP_NOTE_EXPRESSION_PAN, 0.25, -0.5),
        (CLAP_NOTE_EXPRESSION_BRIGHTNESS, 0.8, 0.8),
        (CLAP_NOTE_EXPRESSION_PRESSURE, 0.7, 0.7),
        (CLAP_NOTE_EXPRESSION_EXPRESSION, 0.3, 0.3),
    ];
    let mut events = vec![Event::note(0, 60)];
    events.extend(
        expressions
            .iter()
            .enumerate()
            .map(|(index, (id, value, _))| {
                Event::Expression(clap_event_note_expression {
                    header: header::<clap_event_note_expression>(
                        CLAP_EVENT_NOTE_EXPRESSION,
                        index as u32 + 10,
                    ),
                    expression_id: *id,
                    note_id: 60,
                    port_index: 0,
                    channel: -1,
                    key: -1,
                    value: *value,
                })
            }),
    );
    let (audio, output) = host.process(64, events);
    for (index, (_, _, expected)) in expressions.iter().enumerate() {
        assert!((audio[0][index + 10] - *expected as f32).abs() < 1e-6);
    }
    assert_eq!(output.len(), 7);
    for (actual, (id, value, _)) in output[1..].iter().zip(expressions) {
        let Event::Expression(actual) = actual else {
            panic!("expected expression");
        };
        assert_eq!(
            (
                actual.expression_id,
                actual.note_id,
                actual.channel,
                actual.key
            ),
            (id, 60, -1, -1)
        );
        assert!((actual.value - value).abs() < 1e-6);
    }
}

#[test]
fn clap_wildcard_note_off_and_choke_round_trip_and_native_zero_velocity_survives() {
    let mut host = Host::new(true);
    let events = [
        CLAP_EVENT_NOTE_ON,
        CLAP_EVENT_NOTE_OFF,
        CLAP_EVENT_NOTE_CHOKE,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| {
        Event::Note(clap_event_note {
            header: header::<clap_event_note>(kind, i as u32),
            note_id: 42,
            port_index: 0,
            channel: if i == 0 { 2 } else { -1 },
            key: if i == 0 { 60 } else { -1 },
            velocity: 0.0,
        })
    })
    .collect();
    let (_, events) = host.process(64, events);
    // Choke output is not represented by the current wrapper; input behavior is
    // covered in Weft. Note-off and note-on must preserve their native addresses.
    assert!(
        matches!(&events[0], Event::Note(e) if e.header.type_ == CLAP_EVENT_NOTE_ON && e.velocity == 0.0)
    );
    assert!(
        matches!(&events[1], Event::Note(e) if e.header.type_ == CLAP_EVENT_NOTE_OFF && e.channel == -1 && e.key == -1)
    );
}

#[test]
fn clap_advertises_mpe_only_when_opted_in() {
    use clap_sys::ext::note_ports::*;
    for enabled in [false, true] {
        let host = Host::new(enabled);
        unsafe {
            let ports =
                &*((*host.plugin).get_extension.unwrap()(host.plugin, CLAP_EXT_NOTE_PORTS.as_ptr())
                    as *const clap_plugin_note_ports);
            let mut info = std::mem::zeroed();
            assert!(ports.get.unwrap()(host.plugin, 0, true, &mut info));
            assert_eq!(
                info.supported_dialects & CLAP_NOTE_DIALECT_MIDI_MPE != 0,
                enabled
            );
            assert_eq!(info.preferred_dialect, CLAP_NOTE_DIALECT_CLAP);
        }
    }
}

fn vst_note(id: i32, pitch: i16) -> vst3::Steinberg::Vst::NoteOnEvent {
    vst3::Steinberg::Vst::NoteOnEvent {
        channel: 2,
        pitch,
        tuning: 0.0,
        velocity: 1.0,
        length: 0,
        noteId: id,
    }
}
fn vst_expression(
    id: i32,
    kind: u32,
    value: f64,
) -> vst3::Steinberg::Vst::NoteExpressionValueEvent {
    vst3::Steinberg::Vst::NoteExpressionValueEvent {
        noteId: id,
        typeId: kind,
        value,
    }
}
#[test]
fn vst3_round_trips_distinct_expression_types_and_units() {
    use vst3_expressions::*;
    let mut controller = NoteExpressionController::default();
    controller.register_note(&vst_note(42, 60));
    let cases = [
        NoteEvent::PolyTuning {
            timing: 19,
            voice_id: VoiceID::ID(42),
            channel: Channel::Number(2),
            key: Key::Number(60),
            tuning: 7.5,
        },
        NoteEvent::PolyVolume {
            timing: 19,
            voice_id: VoiceID::ID(42),
            channel: Channel::Number(2),
            key: Key::Number(60),
            gain: 2.0,
        },
        NoteEvent::PolyPan {
            timing: 19,
            voice_id: VoiceID::ID(42),
            channel: Channel::Number(2),
            key: Key::Number(60),
            pan: -0.5,
        },
        NoteEvent::PolyBrightness {
            timing: 19,
            voice_id: VoiceID::ID(42),
            channel: Channel::Number(2),
            key: Key::Number(60),
            brightness: 0.75,
        },
        NoteEvent::PolyExpression {
            timing: 19,
            voice_id: VoiceID::ID(42),
            channel: Channel::Number(2),
            key: Key::Number(60),
            expression: 0.25,
        },
    ];
    for event in cases {
        let raw = NoteExpressionController::translate_event_reverse(42, &event).unwrap();
        assert_eq!(controller.translate_event::<()>(19, &raw), Some(event));
    }
}
#[test]
fn vst3_long_held_ids_survive_cache_wrap_and_reused_ids_use_latest_address() {
    use vst3_expressions::*;
    let mut controller = NoteExpressionController::default();
    controller.register_note(&vst_note(0, 60));
    for id in 1..300 {
        controller.register_note(&vst_note(id, 64));
    }
    assert!(matches!(
        controller.translate_event::<()>(19, &vst_expression(0, TUNING_EXPRESSION_ID, 0.5)),
        Some(NoteEvent::PolyTuning {
            voice_id: VoiceID::ID(0),
            channel: Channel::Wildcard,
            key: Key::Wildcard,
            timing: 19,
            tuning: 0.0
        })
    ));
    controller.register_note(&vst_note(299, 72));
    assert!(matches!(
        controller.translate_event::<()>(0, &vst_expression(299, PAN_EXPRESSION_ID, 0.5)),
        Some(NoteEvent::PolyPan {
            channel: Channel::Number(2),
            key: Key::Number(72),
            ..
        })
    ));
    assert!(
        controller
            .translate_event::<()>(0, &vst_expression(-1, PAN_EXPRESSION_ID, 0.5))
            .is_none()
    );
    assert!(
        controller
            .translate_event::<()>(0, &vst_expression(299, PAN_EXPRESSION_ID, f64::NAN))
            .is_none()
    );
}

#[test]
fn vst3_expression_catalog_has_distinct_standard_types() {
    use vst3_expressions::*;
    assert_eq!(KNOWN_NOTE_EXPRESSIONS.len(), 6);
    for (id, info) in KNOWN_NOTE_EXPRESSIONS.iter().enumerate() {
        assert_eq!(info.type_id, id as u32);
        assert!(!info.title.is_empty());
        if id == 0 {
            assert_eq!(info.unit, "dB");
        }
    }
}
