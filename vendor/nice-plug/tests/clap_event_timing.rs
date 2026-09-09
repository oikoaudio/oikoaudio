//! Exercise sample offsets through the public CLAP entry point, without a DAW or product DSP.
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
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
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
            let _ = context.try_send_event(event);
        }
        ProcessStatus::Normal
    }
}
impl<const SAMPLE_ACCURATE: bool> ClapPlugin for TimingProbe<SAMPLE_ACCURATE> {
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
enum Event {
    Value(clap_event_param_value),
    Modulation(clap_event_param_mod),
    Note(clap_event_note),
    Transport(clap_event_transport),
}
impl Event {
    fn header(&self) -> &clap_event_header {
        match self {
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
        let notes = &mut *((*list).ctx as *mut Vec<(u32, i16)>);
        if (*event).type_ == CLAP_EVENT_NOTE_ON {
            if notes.len() == notes.capacity() {
                return false;
            }
            notes.push(((*event).time, (*(event as *const clap_event_note)).key));
        }
    }
    true
}

struct Host {
    plugin: *const clap_plugin,
    param_id: u32,
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
                param_id: info.id,
                _host: host,
            }
        }
    }

    fn process(
        &mut self,
        samples: usize,
        mut events: Vec<Event>,
    ) -> ([Vec<f32>; 2], Vec<(u32, i16)>) {
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
        let mut notes = Vec::<(u32, i16)>::with_capacity(32);
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
fn first_automation_event_applies_at_its_offset() {
    for samples in [1, 16, 256] {
        for time in [0, (samples - 1) as u32 / 2, (samples - 1) as u32] {
            let mut host = Host::new(true);
            let (audio, _) = host.process(samples, vec![Event::value(host.param_id, time, 0.5)]);
            assert!(
                audio[0][..time as usize].iter().all(|&value| value == 0.0),
                "early automation at {time} in {samples} samples"
            );
            assert!(audio[0][time as usize..].iter().all(|&value| value == 0.5));
        }
    }
}

#[test]
fn first_modulation_event_applies_at_its_offset() {
    let mut host = Host::new(true);
    let (audio, _) = host.process(256, vec![Event::modulation(host.param_id, 137, 0.25)]);
    assert_eq!(&audio[0][..137], &[0.0; 137]);
    assert_eq!(&audio[0][137..], &[0.25; 119]);
}

#[test]
fn multiple_changes_and_same_sample_groups_preserve_note_offsets() {
    let mut host = Host::new(true);
    let id = host.param_id;
    let (audio, notes) = host.process(
        256,
        vec![
            Event::value(id, 37, 0.25),
            Event::note(37, 60),
            Event::value(id, 137, 0.5),
            Event::modulation(id, 137, 0.25),
            Event::note(137, 64),
            Event::note(255, 67),
        ],
    );
    assert_eq!(&audio[0][..37], &[0.0; 37]);
    assert_eq!(&audio[0][37..137], &[0.25; 100]);
    assert_eq!(&audio[0][137..], &[0.75; 119]);
    assert_eq!(notes, [(37, 60), (137, 64), (255, 67)]);
}

#[test]
fn leading_note_does_not_hide_a_later_parameter_change() {
    let mut host = Host::new(true);
    let (audio, notes) = host.process(
        256,
        vec![Event::note(19, 60), Event::value(host.param_id, 137, 0.5)],
    );
    assert_eq!(&audio[0][..137], &[0.0; 137]);
    assert_eq!(&audio[0][137..], &[0.5; 119]);
    assert_eq!(notes, [(19, 60)]);
}

#[test]
fn empty_lists_and_notes_only_preserve_state_and_timing() {
    let mut host = Host::new(true);
    let (audio, notes) = host.process(256, vec![]);
    assert_eq!(audio[0], vec![0.0; 256]);
    assert!(notes.is_empty());
    let (audio, notes) = host.process(256, vec![Event::note(19, 60), Event::note(255, 64)]);
    assert_eq!(audio[0], vec![0.0; 256]);
    assert_eq!(notes, [(19, 60), (255, 64)]);
}

#[test]
fn disabling_sample_accurate_automation_keeps_block_rate_behavior() {
    let mut host = Host::new(false);
    let (audio, _) = host.process(256, vec![Event::value(host.param_id, 137, 0.5)]);
    assert_eq!(audio[0], vec![0.5; 256]);
}

#[test]
fn first_transport_event_splits_even_without_sample_accurate_automation() {
    for sample_accurate in [true, false] {
        let mut host = Host::new(sample_accurate);
        let (audio, _) = host.process(256, vec![Event::tempo(137, 90.0)]);
        assert_eq!(&audio[1][..137], &[120.0; 137]);
        assert_eq!(&audio[1][137..], &[90.0; 119]);
    }
}

#[test]
fn automation_timeline_is_independent_of_host_block_size() {
    let changes = [
        (17, 0.125),
        (137, 0.5),
        (255, 0.75),
        (256, 0.25),
        (383, 1.0),
    ];
    let expected: Vec<f32> = (0..512)
        .map(|sample| {
            changes
                .iter()
                .rev()
                .find(|&&(time, _)| time <= sample)
                .map_or(0.0, |&(_, value)| value as f32)
        })
        .collect();
    for block_size in [1, 16, 64, 256] {
        let mut host = Host::new(true);
        let mut rendered = Vec::new();
        for start in (0..512).step_by(block_size) {
            let events = changes
                .iter()
                .filter(|&&(time, _)| (start..start + block_size).contains(&time))
                .map(|&(time, value)| Event::value(host.param_id, (time - start) as u32, value))
                .collect();
            let (audio, _) = host.process(block_size, events);
            rendered.extend_from_slice(&audio[0]);
        }
        assert_eq!(rendered, expected, "host block size {block_size}");
    }
}
