//! State loading through the public CLAP API, including allocation failure injection.
// `assert_process_allocs` installs the wrapper's own `#[global_allocator]`, which would conflict
// with this test's allocator.
#![cfg(not(feature = "assert_process_allocs"))]
use clap_sys::{
    ext::{
        params::{CLAP_EXT_PARAMS, clap_param_info, clap_plugin_params},
        state::{CLAP_EXT_STATE, clap_plugin_state},
    },
    factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory},
    host::clap_host,
    stream::clap_istream,
    version::CLAP_VERSION,
};
use nice_plug::prelude::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    ffi::{c_char, c_void},
    ptr,
    sync::Arc,
};

// Only the calling test thread is affected. Never exhaust the machine to test OOM.
thread_local! {
    static FAIL_LARGE_ALLOCATION: Cell<bool> = const { Cell::new(false) };
    static LARGEST_ALLOCATION: Cell<usize> = const { Cell::new(0) };
    static DENIED_ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct TestAllocator;
fn deny(size: usize) -> bool {
    LARGEST_ALLOCATION.with(|largest| largest.set(largest.get().max(size)));
    let deny = FAIL_LARGE_ALLOCATION.with(|fail| fail.get() && size >= 128 * 1024);
    if deny {
        DENIED_ALLOCATIONS.with(|count| count.set(count.get() + 1));
    }
    deny
}
unsafe impl GlobalAlloc for TestAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if deny(layout.size()) {
            ptr::null_mut()
        } else {
            unsafe { System.alloc(layout) }
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if deny(layout.size()) {
            ptr::null_mut()
        } else {
            unsafe { System.alloc_zeroed(layout) }
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if deny(size) {
            ptr::null_mut()
        } else {
            unsafe { System.realloc(ptr, layout, size) }
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: TestAllocator = TestAllocator;

#[derive(Params)]
struct StateParams {
    #[id = "value"]
    value: IntParam,
}
impl Default for StateParams {
    fn default() -> Self {
        Self {
            value: IntParam::new("Value", 0, IntRange::Linear { min: 0, max: 10 }),
        }
    }
}
#[derive(Default)]
struct StateProbe {
    params: Arc<StateParams>,
}
impl Plugin for StateProbe {
    const NAME: &'static str = "State probe";
    const VENDOR: &'static str = "NicePlug tests";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = "0.0.0";
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];
    type Editor = ();
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn process(
        &mut self,
        _: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        _: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        ProcessStatus::Normal
    }
}
impl ClapPlugin for StateProbe {
    const CLAP_ID: &'static str = "test.state-stream";
    const CLAP_DESCRIPTION: Option<&'static str> = None;
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Utility];
}
nice_export_clap!(StateProbe);

// Generate padding in the callback so the large-state test needs only one large buffer.
// A valid JSON document followed by whitespace exercises the same envelope size as
// a large sample payload without allocating a second huge deserialized object.
const JSON: &[u8] = br#"{"version":"0.0.0","params":{"value":{"i32":7}},"fields":{}}"#;
struct Input {
    declared: u64,
    available: usize,
    position: usize,
    max_read: usize,
    end_result: i64,
}
unsafe extern "C" fn read(stream: *const clap_istream, data: *mut c_void, size: u64) -> i64 {
    let input = unsafe { &mut *((*stream).ctx as *mut Input) };
    let remaining = (8 + input.available).saturating_sub(input.position);
    if remaining == 0 {
        return input.end_result;
    }
    let count = remaining.min(size as usize).min(input.max_read);
    let destination = unsafe { std::slice::from_raw_parts_mut(data.cast::<u8>(), count) };
    let prefix = input.declared.to_le_bytes();
    // Most reads are padding; fill those without a byte-by-byte loop in debug tests.
    destination.fill(b' ');
    for (index, byte) in destination
        .iter_mut()
        .enumerate()
        .take((8 + JSON.len()).saturating_sub(input.position))
    {
        let offset = input.position + index;
        *byte = if offset < 8 {
            prefix[offset]
        } else {
            JSON[offset - 8]
        };
    }
    input.position += count;
    count as i64
}
unsafe extern "C" fn extension(_: *const clap_host, _: *const c_char) -> *const c_void {
    ptr::null()
}

fn load(input: &mut Input, fail_allocation: bool) -> (bool, usize, usize) {
    let host = clap_host {
        clap_version: CLAP_VERSION,
        host_data: ptr::null_mut(),
        name: c"State test host".as_ptr(),
        vendor: c"NicePlug tests".as_ptr(),
        url: c"".as_ptr(),
        version: c"1".as_ptr(),
        get_extension: Some(extension),
        request_restart: None,
        request_process: None,
        request_callback: None,
    };
    unsafe {
        assert!(clap_entry.init.unwrap()(c"test".as_ptr()));
        let factory = &*(clap_entry.get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr())
            as *const clap_plugin_factory);
        let plugin = factory.create_plugin.unwrap()(factory, &host, c"test.state-stream".as_ptr());
        assert!(!plugin.is_null());
        assert!((*plugin).init.unwrap()(plugin));
        let state = &*((*plugin).get_extension.unwrap()(plugin, CLAP_EXT_STATE.as_ptr())
            as *const clap_plugin_state);
        let stream = clap_istream {
            ctx: (input as *mut Input).cast(),
            read: Some(read),
        };
        LARGEST_ALLOCATION.with(|v| v.set(0));
        DENIED_ALLOCATIONS.with(|v| v.set(0));
        FAIL_LARGE_ALLOCATION.with(|v| v.set(fail_allocation));
        let success = state.load.unwrap()(plugin, &stream);
        FAIL_LARGE_ALLOCATION.with(|v| v.set(false));
        let largest = LARGEST_ALLOCATION.with(Cell::get);
        let denied = DENIED_ALLOCATIONS.with(Cell::get);
        let params = &*((*plugin).get_extension.unwrap()(plugin, CLAP_EXT_PARAMS.as_ptr())
            as *const clap_plugin_params);
        let mut info = std::mem::zeroed::<clap_param_info>();
        assert!(params.get_info.unwrap()(plugin, 0, &mut info));
        let mut value = -1.0;
        assert!(params.get_value.unwrap()(plugin, info.id, &mut value));
        let expected = if success { 7.0 } else { 0.0 };
        // CLAP's plain value passes through a normalized f32 in this wrapper.
        assert!(
            (value - expected).abs() < 1e-6,
            "state value {value}, expected {expected}"
        );
        (*plugin).destroy.unwrap()(plugin);
        clap_entry.deinit.unwrap()();
        (success, largest, denied)
    }
}
fn input(declared: u64, available: usize) -> Input {
    Input {
        declared,
        available,
        position: 0,
        max_read: usize::MAX,
        end_result: 0,
    }
}

#[test]
fn valid_states_cross_chunk_boundaries_and_preserve_envelope_end() {
    for length in [JSON.len(), 65535, 65536, 65537, 3 * 65536 + 17] {
        let mut input = input(length as u64, length + 13);
        input.max_read = 997;
        assert!(load(&mut input, false).0, "length {length}");
        assert_eq!(
            input.position,
            8 + length,
            "must not consume the next envelope"
        );
    }
}
#[test]
fn valid_state_larger_than_previous_cap() {
    let length = 256 * 1024 * 1024 + 1;
    let mut input = input(length as u64, length);
    assert!(load(&mut input, false).0);
    assert_eq!(input.position, 8 + length);
}
#[test]
fn huge_truncated_state_does_not_reserve_declared_length() {
    for declared in [u64::MAX, isize::MAX as u64, 8_028_074_745_930_326_651] {
        let mut input = input(declared, JSON.len());
        let (success, largest, _) = load(&mut input, false);
        assert!(!success);
        assert!(
            largest <= 65536,
            "requested {largest} for only a few input bytes"
        );
    }
}
#[test]
fn stream_errors_and_truncation_fail() {
    for available in [0, JSON.len(), 65536 + 19] {
        for error in [0, -1] {
            let mut input = input(3 * 65536, available);
            input.max_read = 7;
            input.end_result = error;
            assert!(!load(&mut input, false).0);
        }
    }
}
#[test]
fn allocation_failure_returns_false_without_aborting() {
    let mut input = input(3 * 65536, 3 * 65536);
    let (success, _, denied) = load(&mut input, true);
    assert!(!success);
    assert!(denied > 0, "must exercise a refused allocation");
    assert!(input.position < 8 + input.available);
}

#[test]
fn stream_overreport_is_rejected() {
    let mut input = input(65536, JSON.len());
    // The callback claims more bytes than requested but does not actually write them.
    input.end_result = i64::MAX;
    assert!(!load(&mut input, false).0);
}
