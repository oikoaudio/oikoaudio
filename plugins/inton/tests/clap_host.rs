use clap_sys::{
    entry::*, events::*, ext::params::*, factory::plugin_factory::*, host::*, plugin::*, version::*,
};
use std::{
    ffi::{c_char, c_void},
    ptr,
};
unsafe extern "C" fn host_ext(_: *const clap_host, _: *const c_char) -> *const c_void {
    ptr::null()
}
fn host() -> clap_host {
    clap_host {
        clap_version: CLAP_VERSION,
        host_data: ptr::null_mut(),
        name: c"Inton test host".as_ptr(),
        vendor: c"Oiko".as_ptr(),
        url: c"".as_ptr(),
        version: c"1".as_ptr(),
        get_extension: Some(host_ext),
        request_restart: None,
        request_process: None,
        request_callback: None,
    }
}
unsafe fn create(h: &clap_host) -> *const clap_plugin {
    static LIB: std::sync::OnceLock<libloading::Library> = std::sync::OnceLock::new();
    let entry: &clap_plugin_entry = if let Some(path) = std::env::var_os("INTON_TEST_CLAP") {
        let lib = LIB.get_or_init(|| libloading::Library::new(path).unwrap());
        &**lib
            .get::<*const clap_plugin_entry>(b"clap_entry\0")
            .unwrap()
    } else {
        &inton::plugin::clap_entry
    };
    assert!(entry.init.unwrap()(c"test".as_ptr()));
    let f = &*(entry.get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr())
        as *const clap_plugin_factory);
    assert_eq!(f.get_plugin_count.unwrap()(f), 1);
    let desc = f.get_plugin_descriptor.unwrap()(f, 0);
    assert_eq!(
        std::ffi::CStr::from_ptr((*desc).name).to_str().unwrap(),
        "Oiko Inton"
    );
    let p = f.create_plugin.unwrap()(f, h, (*desc).id);
    assert!(!p.is_null());
    assert!((*p).init.unwrap()(p));
    p
}
unsafe fn parameter_ids(params: &clap_plugin_params, plugin: *const clap_plugin) -> [u32; 5] {
    let mut ids = [0; 5];
    for index in 0..5 {
        let mut info = std::mem::zeroed::<clap_param_info>();
        assert!(params.get_info.unwrap()(plugin, index as u32, &mut info));
        let name = std::ffi::CStr::from_ptr(info.name.as_ptr())
            .to_str()
            .unwrap();
        let position = inton::plugin::PARAM_NAMES
            .iter()
            .position(|expected| *expected == name)
            .unwrap();
        ids[position] = info.id;
    }
    ids
}
unsafe extern "C" fn size(list: *const clap_input_events) -> u32 {
    (*((*list).ctx as *const Vec<clap_event_param_value>)).len() as u32
}
unsafe extern "C" fn get(list: *const clap_input_events, index: u32) -> *const clap_event_header {
    &(&*((*list).ctx as *const Vec<clap_event_param_value>))[index as usize].header
}
#[test]
fn actual_clap_metadata_truncation_batch_and_text() {
    unsafe {
        let h = host();
        let p = create(&h);
        let params = &*((*p).get_extension.unwrap()(p, CLAP_EXT_PARAMS.as_ptr())
            as *const clap_plugin_params);
        assert_eq!(params.count.unwrap()(p), 5);
        let ids = parameter_ids(params, p);
        for (index, id) in ids.into_iter().enumerate() {
            let mut info = std::mem::zeroed();
            assert!(params.get_info.unwrap()(p, index as u32, &mut info));
            assert_eq!(info.id, id);
            assert!(info.flags & CLAP_PARAM_IS_AUTOMATABLE != 0);
            assert!(info.flags & CLAP_PARAM_IS_MODULATABLE != 0);
            if index == 0 {
                assert!(info.flags & CLAP_PARAM_IS_STEPPED != 0);
                assert_eq!(info.max_value, 31.);
            }
        }
        let mut events = vec![];
        for (id, value) in [(ids[0], 1.9), (ids[2], 432.), (ids[3], 12.)] {
            events.push(clap_event_param_value {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_param_value>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_PARAM_VALUE,
                    flags: 0,
                },
                param_id: id,
                cookie: ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value,
            });
        }
        let list = clap_input_events {
            ctx: (&mut events as *mut Vec<_>).cast(),
            size: Some(size),
            get: Some(get),
        };
        params.flush.unwrap()(p, &list, ptr::null());
        let mut v = 0.;
        assert!(params.get_value.unwrap()(p, ids[0], &mut v));
        assert!((v - 2.0).abs() < 1e-5);
        let mut text = [0i8; 100];
        assert!(params.value_to_text.unwrap()(
            p,
            ids[0],
            2.0,
            text.as_mut_ptr(),
            100
        ));
        assert_eq!(
            std::ffi::CStr::from_ptr(text.as_ptr()).to_str().unwrap(),
            "3: Empty"
        );
        assert!(params.text_to_value.unwrap()(
            p,
            ids[0],
            c"1: 12 EDO".as_ptr(),
            &mut v
        ));
        assert_eq!(v, 0.);
        (*p).destroy.unwrap()(p);
    }
}
struct Writer {
    bytes: Vec<u8>,
}
unsafe extern "C" fn write_stream(
    stream: *const clap_sys::stream::clap_ostream,
    data: *const c_void,
    size: u64,
) -> i64 {
    let w = &mut *((*stream).ctx as *mut Writer);
    let count = (size as usize).min(7);
    w.bytes
        .extend_from_slice(std::slice::from_raw_parts(data.cast::<u8>(), count));
    count as i64
}
struct Reader {
    bytes: Vec<u8>,
    offset: usize,
}
fn decode_state(bytes: &[u8]) -> serde_json::Value {
    let length = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    serde_json::from_slice(&bytes[8..8 + length]).unwrap()
}
fn encode_state(value: &serde_json::Value) -> Vec<u8> {
    let json = serde_json::to_vec(value).unwrap();
    let mut bytes = (json.len() as u64).to_le_bytes().to_vec();
    bytes.extend(json);
    bytes
}
unsafe extern "C" fn read_stream(
    stream: *const clap_sys::stream::clap_istream,
    data: *mut c_void,
    size: u64,
) -> i64 {
    let r = &mut *((*stream).ctx as *mut Reader);
    let count = (size as usize).min(11).min(r.bytes.len() - r.offset);
    ptr::copy_nonoverlapping(r.bytes.as_ptr().add(r.offset), data.cast(), count);
    r.offset += count;
    count as i64
}
#[test]
fn real_state_stream_partial_reads_portability_and_corruption() {
    use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
    unsafe extern "C" fn rescan(host: *const clap_host, flags: u32) {
        let received = &*((*host).host_data as *const AtomicU32);
        received.fetch_or(flags, SeqCst);
    }
    static HOST_PARAMS: clap_host_params = clap_host_params {
        rescan: Some(rescan),
        clear: None,
        request_flush: None,
    };
    unsafe extern "C" fn extensions(_: *const clap_host, id: *const c_char) -> *const c_void {
        if std::ffi::CStr::from_ptr(id) == CLAP_EXT_PARAMS {
            (&HOST_PARAMS as *const clap_host_params).cast()
        } else {
            ptr::null()
        }
    }
    unsafe {
        use clap_sys::{ext::state::*, stream::*};
        let rescanned = AtomicU32::new(0);
        let h = clap_host {
            host_data: (&rescanned as *const AtomicU32).cast_mut().cast(),
            get_extension: Some(extensions),
            ..host()
        };
        let p = create(&h);
        let state =
            &*((*p).get_extension.unwrap()(p, CLAP_EXT_STATE.as_ptr()) as *const clap_plugin_state);
        let mut writer = Writer { bytes: vec![] };
        let output = clap_ostream {
            ctx: (&mut writer as *mut Writer).cast(),
            write: Some(write_stream),
        };
        assert!(state.save.unwrap()(p, &output));
        let mut envelope = decode_state(&writer.bytes);
        let mut project: inton_core::state::Project =
            serde_json::from_str(envelope["fields"]["inton-project-v1"].as_str().unwrap()).unwrap();
        let library = inton::library::Library::factory();
        project
            .assign(7, library.load("factory:13ed8").unwrap())
            .unwrap();
        project.assign(3, inton_core::tuning::twelve_edo()).unwrap();
        project.clear(3).unwrap();
        project.parameters.position = 7;
        project.parameters.reference = 432.;
        project.parameters.transpose = -2;
        project.parameters.enabled = false;
        envelope["params"]["set_position"] = serde_json::json!({"i32": 7});
        envelope["params"]["reference_frequency"] = serde_json::json!({"f32": 432.0});
        envelope["params"]["transpose"] = serde_json::json!({"i32": -2});
        envelope["params"]["mts_enabled"] = serde_json::json!({"bool": false});
        envelope["fields"]["inton-project-v1"] =
            serde_json::Value::String(serde_json::to_string(&project).unwrap());
        let mut reader = Reader {
            bytes: encode_state(&envelope),
            offset: 0,
        };
        let input = clap_istream {
            ctx: (&mut reader as *mut Reader).cast(),
            read: Some(read_stream),
        };
        assert!(state.load.unwrap()(p, &input));
        assert_ne!(
            rescanned.swap(0, SeqCst) & CLAP_PARAM_RESCAN_VALUES,
            0,
            "Loading state must invalidate the host's cached parameter values"
        );
        writer.bytes.clear();
        assert!(state.save.unwrap()(p, &output));
        let recalled_envelope = decode_state(&writer.bytes);
        let recalled: inton_core::state::Project = serde_json::from_str(
            recalled_envelope["fields"]["inton-project-v1"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(recalled.parameters, project.parameters);
        assert_eq!(recalled.slots[7], project.slots[7]);
        assert!(recalled.has_slot(3) && recalled.slots[3].is_none());
        reader.bytes = encode_state(&serde_json::json!({"broken": true}));
        reader.offset = 0;
        assert!(!state.load.unwrap()(p, &input));
        assert_eq!(rescanned.load(SeqCst), 0);
        writer.bytes.clear();
        assert!(state.save.unwrap()(p, &output));
        let after_error = decode_state(&writer.bytes);
        let after_error: inton_core::state::Project =
            serde_json::from_str(after_error["fields"]["inton-project-v1"].as_str().unwrap())
                .unwrap();
        assert_eq!(after_error.parameters, project.parameters);

        // Missing envelopes, unrepresentable lengths, and truncated large envelopes fail
        // without replacing the project or reserving their entire claimed size.
        for bytes in [
            project.encode().unwrap(),
            u64::MAX.to_le_bytes().to_vec(),
            (256_u64 * 1024 * 1024 + 1).to_le_bytes().to_vec(),
        ] {
            reader.bytes = bytes;
            reader.offset = 0;
            assert!(!state.load.unwrap()(p, &input));
            assert_eq!(rescanned.load(SeqCst), 0);
        }
        writer.bytes.clear();
        assert!(state.save.unwrap()(p, &output));
        let after_rejection = decode_state(&writer.bytes);
        let after_rejection: inton_core::state::Project = serde_json::from_str(
            after_rejection["fields"]["inton-project-v1"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            after_rejection.encode().unwrap(),
            after_error.encode().unwrap()
        );
        (*p).destroy.unwrap()(p);
    }
}
unsafe extern "C" fn push_output(
    out: *const clap_output_events,
    event: *const clap_event_header,
) -> bool {
    let data = &mut *((*out).ctx as *mut Vec<(u16, u32)>);
    data.push(((*event).type_, (*event).time));
    true
}
unsafe extern "C" fn midi_size(_: *const clap_input_events) -> u32 {
    1
}
unsafe extern "C" fn midi_get(list: *const clap_input_events, _: u32) -> *const clap_event_header {
    (*list).ctx.cast()
}
#[test]
fn transparent_audio_midi_and_zero_callback_allocations() {
    unsafe {
        use clap_sys::{audio_buffer::*, process::*};
        let h = host();
        let p = create(&h);
        assert!((*p).activate.unwrap()(p, 48000., 1, 256));
        assert!((*p).start_processing.unwrap()(p));
        let mut left = [0.125f32; 256];
        let mut right = [-0.75f32; 256];
        let mut out_l = [0f32; 256];
        let mut out_r = [0f32; 256];
        let mut inputs = [left.as_mut_ptr(), right.as_mut_ptr()];
        let mut outputs = [out_l.as_mut_ptr(), out_r.as_mut_ptr()];
        let input = clap_audio_buffer {
            data32: inputs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 3,
        };
        let mut output = clap_audio_buffer {
            data32: outputs.as_mut_ptr(),
            ..input
        };
        let mut note = clap_event_midi {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_midi>() as u32,
                time: 19,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_MIDI,
                flags: 0,
            },
            port_index: 0,
            data: [0x90, 60, 99],
        };
        let notes = clap_input_events {
            ctx: (&mut note as *mut clap_event_midi).cast(),
            size: Some(midi_size),
            get: Some(midi_get),
        };
        let mut received = Vec::<(u16, u32)>::with_capacity(1);
        let out_notes = clap_output_events {
            ctx: (&mut received as *mut Vec<_>).cast(),
            try_push: Some(push_output),
        };
        let mut process = clap_process {
            steady_time: 0,
            frames_count: 256,
            transport: ptr::null(),
            audio_inputs: &input,
            audio_outputs: &mut output,
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: &notes,
            out_events: &out_notes,
        };
        let status = (*p).process.unwrap()(p, &process);
        assert_eq!(status, CLAP_PROCESS_CONTINUE_IF_NOT_QUIET);
        assert_eq!(out_l, left);
        assert_eq!(out_r, right);
        assert_eq!(received.len(), 1);
        assert!(matches!(
            received[0].0,
            CLAP_EVENT_MIDI | CLAP_EVENT_NOTE_ON
        ));
        assert_eq!(received[0].1, 19);
        // Hosts may invoke utilities with no connected buses.
        process.audio_inputs_count = 0;
        process.audio_outputs_count = 0;
        process.in_events = ptr::null();
        process.out_events = ptr::null();
        assert_eq!(
            (*p).process.unwrap()(p, &process),
            CLAP_PROCESS_CONTINUE_IF_NOT_QUIET
        );
        (*p).stop_processing.unwrap()(p);
        (*p).deactivate.unwrap()(p);
        (*p).destroy.unwrap()(p);
    }
}
#[test]
#[ignore = "Requires INTON_MTS_LIBRARY and an idle MTS environment; optionally INTON_TEST_CLAP for the built binary"]
fn built_clap_automates_official_mts_without_an_editor() {
    unsafe {
        use clap_sys::{ext::state::*, stream::*};
        struct Cleanup(*const clap_plugin);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                unsafe { (*self.0).destroy.unwrap()(self.0) }
            }
        }
        let library =
            libloading::Library::new(std::env::var_os("INTON_MTS_LIBRARY").unwrap()).unwrap();
        let has: libloading::Symbol<unsafe extern "C" fn() -> bool> =
            library.get(b"MTS_HasMaster\0").unwrap();
        let table: libloading::Symbol<unsafe extern "C" fn() -> *const f64> =
            library.get(b"MTS_GetTuningTable\0").unwrap();
        assert!(
            !has(),
            "An existing MTS master must not be disturbed by tests"
        );
        let h = host();
        let p = create(&h);
        let cleanup = Cleanup(p);
        let state =
            &*((*p).get_extension.unwrap()(p, CLAP_EXT_STATE.as_ptr()) as *const clap_plugin_state);
        let params = &*((*p).get_extension.unwrap()(p, CLAP_EXT_PARAMS.as_ptr())
            as *const clap_plugin_params);
        let ids = parameter_ids(params, p);
        let factory = inton::library::Library::factory();
        let mut project = inton_core::state::Project::default();
        project
            .assign(0, factory.load("factory:22edo").unwrap())
            .unwrap();
        project
            .assign(1, factory.load("factory:13ed3").unwrap())
            .unwrap();
        project
            .assign(2, factory.load("factory:13ed8").unwrap())
            .unwrap();
        let mut writer = Writer { bytes: vec![] };
        let output = clap_ostream {
            ctx: (&mut writer as *mut Writer).cast(),
            write: Some(write_stream),
        };
        assert!(state.save.unwrap()(p, &output));
        let mut envelope = decode_state(&writer.bytes);
        envelope["fields"]["inton-project-v1"] =
            serde_json::Value::String(serde_json::to_string(&project).unwrap());
        let mut reader = Reader {
            bytes: encode_state(&envelope),
            offset: 0,
        };
        let stream = clap_istream {
            ctx: (&mut reader as *mut Reader).cast(),
            read: Some(read_stream),
        };
        assert!(state.load.unwrap()(p, &stream));
        let wait_for = |expected: f64| {
            let started = std::time::Instant::now();
            loop {
                if has() && (*table().add(60) / expected - 1.).abs() < 1e-8 {
                    break;
                }
                assert!(started.elapsed() < std::time::Duration::from_secs(2));
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        let a = project.slots[0].as_ref().unwrap().prepare().unwrap().hz[60];
        wait_for(a);
        let send = |values: &[(u32, f64)]| {
            let mut events = values
                .iter()
                .map(|(id, value)| clap_event_param_value {
                    header: clap_event_header {
                        size: std::mem::size_of::<clap_event_param_value>() as u32,
                        time: 0,
                        space_id: CLAP_CORE_EVENT_SPACE_ID,
                        type_: CLAP_EVENT_PARAM_VALUE,
                        flags: 0,
                    },
                    param_id: *id,
                    cookie: ptr::null_mut(),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: *value,
                })
                .collect::<Vec<_>>();
            let list = clap_input_events {
                ctx: (&mut events as *mut Vec<_>).cast(),
                size: Some(size),
                get: Some(get),
            };
            params.flush.unwrap()(p, &list, ptr::null());
        };
        send(&[(ids[0], 1.), (ids[2], 432.), (ids[3], 12.)]);
        let b = project.slots[1].as_ref().unwrap().prepare().unwrap().hz[60] * 432. / 440. * 2.;
        wait_for(b);
        send(&[(ids[0], 2.), (ids[1], 300.)]);
        let c = project.slots[2].as_ref().unwrap().prepare().unwrap().hz[60] * 432. / 440. * 2.;
        std::thread::sleep(std::time::Duration::from_millis(140));
        let middle = *table().add(60);
        assert!(middle > b.min(c) && middle < b.max(c));
        wait_for(c);
        send(&[(ids[0], 31.)]);
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!((*table().add(60) / c - 1.).abs() < 1e-8);
        send(&[(ids[4], 0.)]);
        let started = std::time::Instant::now();
        while has() {
            assert!(started.elapsed() < std::time::Duration::from_secs(2));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        drop(cleanup);
        assert!(!has());
    }
}

#[test]
fn gui_discovery_exposes_complete_interface() {
    use clap_sys::ext::gui::*;
    unsafe {
        let h = host();
        let p = create(&h);
        let raw = (*p).get_extension.unwrap()(p, CLAP_EXT_GUI.as_ptr());
        assert!(
            !raw.is_null(),
            "Host must discover the GUI before creating a window"
        );
        let gui = &*(raw as *const clap_plugin_gui);
        // Hosts may reject an incomplete extension before showing an editor button,
        // including callbacks for capabilities this fixed, embedded editor declines.
        let callbacks = [
            ("is_api_supported", gui.is_api_supported.is_some()),
            ("get_preferred_api", gui.get_preferred_api.is_some()),
            ("create", gui.create.is_some()),
            ("destroy", gui.destroy.is_some()),
            ("set_scale", gui.set_scale.is_some()),
            ("get_size", gui.get_size.is_some()),
            ("can_resize", gui.can_resize.is_some()),
            ("get_resize_hints", gui.get_resize_hints.is_some()),
            ("adjust_size", gui.adjust_size.is_some()),
            ("set_size", gui.set_size.is_some()),
            ("set_parent", gui.set_parent.is_some()),
            ("set_transient", gui.set_transient.is_some()),
            ("suggest_title", gui.suggest_title.is_some()),
            ("show", gui.show.is_some()),
            ("hide", gui.hide.is_some()),
        ];
        let missing: Vec<_> = callbacks
            .iter()
            .filter(|(_, present)| !present)
            .map(|(name, _)| *name)
            .collect();
        // Release the worker even if the discovery assertion fails.
        if !missing.is_empty() {
            (*p).destroy.unwrap()(p);
            panic!("Incomplete CLAP GUI extension: {}", missing.join(", "));
        }
        let (mut api, mut floating) = (ptr::null(), true);
        assert!(gui.get_preferred_api.unwrap()(p, &mut api, &mut floating));
        assert!(!floating);
        #[cfg(target_os = "linux")]
        assert_eq!(std::ffi::CStr::from_ptr(api), CLAP_WINDOW_API_X11);
        #[cfg(target_os = "macos")]
        assert_eq!(std::ffi::CStr::from_ptr(api), CLAP_WINDOW_API_COCOA);
        #[cfg(target_os = "windows")]
        assert_eq!(std::ffi::CStr::from_ptr(api), CLAP_WINDOW_API_WIN32);
        assert!(gui.is_api_supported.unwrap()(p, api, floating));
        assert!(!gui.is_api_supported.unwrap()(p, api, true));
        assert!(!gui.can_resize.unwrap()(p));
        let mut hints: clap_gui_resize_hints = std::mem::zeroed();
        assert!(!gui.get_resize_hints.unwrap()(p, &mut hints));
        assert!(!gui.get_resize_hints.unwrap()(p, ptr::null_mut()));
        assert!(!gui.set_transient.unwrap()(p, ptr::null()));
        gui.suggest_title.unwrap()(p, c"Host title".as_ptr());
        (*p).destroy.unwrap()(p);
    }
}

#[test]
fn invalid_project_rejects_entire_host_state() {
    unsafe {
        use clap_sys::{ext::state::*, stream::*};
        let h = host();
        let p = create(&h);
        let state =
            &*((*p).get_extension.unwrap()(p, CLAP_EXT_STATE.as_ptr()) as *const clap_plugin_state);
        let params = &*((*p).get_extension.unwrap()(p, CLAP_EXT_PARAMS.as_ptr())
            as *const clap_plugin_params);
        let ids = parameter_ids(params, p);
        let mut writer = Writer { bytes: vec![] };
        let output = clap_ostream {
            ctx: (&mut writer as *mut Writer).cast(),
            write: Some(write_stream),
        };
        assert!(state.save.unwrap()(p, &output));
        let saved = decode_state(&writer.bytes);
        for corrupt in 0..5 {
            let mut envelope = saved.clone();
            let mut project: inton_core::state::Project =
                serde_json::from_str(envelope["fields"]["inton-project-v1"].as_str().unwrap())
                    .unwrap();
            match corrupt {
                0 => project.slots[0].as_mut().unwrap().scl_text = "invalid scale".into(),
                1 => project.slots.clear(),
                2 => project.parameters.position = 32,
                3 => project.held_log = Some(vec![0.0; 127]),
                _ => {}
            }
            envelope["params"]["reference_frequency"] = serde_json::json!({"f32":432.0});
            envelope["fields"]["inton-project-v1"] =
                serde_json::Value::String(serde_json::to_string(&project).unwrap());
            if corrupt == 4 {
                envelope["params"]["transpose"] = serde_json::json!({"i32":1000});
            }
            let mut reader = Reader {
                bytes: encode_state(&envelope),
                offset: 0,
            };
            let input = clap_istream {
                ctx: (&mut reader as *mut Reader).cast(),
                read: Some(read_stream),
            };
            assert!(
                !state.load.unwrap()(p, &input),
                "Accepted corrupt state {corrupt}"
            );
            let mut reference = 0.0;
            assert!(params.get_value.unwrap()(p, ids[2], &mut reference));
            assert!(
                (reference - 0.5).abs() < 1e-6,
                "Rejected state changed reference to {reference}"
            );
            writer.bytes.clear();
            assert!(state.save.unwrap()(p, &output));
            assert_eq!(
                decode_state(&writer.bytes),
                saved,
                "Rejected state changed live state"
            );
        }
        (*p).destroy.unwrap()(p);
    }
}
