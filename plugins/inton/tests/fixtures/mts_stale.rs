//! Private fake MTS ABI for the dynamic recovery integration test.
#![allow(non_snake_case, static_mut_refs)]

use std::{ffi::c_void, ptr};

static mut MASTER: bool = true;
static mut RESETS: i32 = 0;
static mut WRITES: i32 = 0;
static mut TUNING: [f64; 128] = [0.0; 128];

#[no_mangle]
pub unsafe extern "C" fn MTS_HasMaster() -> bool {
    MASTER
}
#[no_mangle]
pub unsafe extern "C" fn MTS_HasIPC() -> bool {
    true
}
#[no_mangle]
pub unsafe extern "C" fn MTS_Reinitialize() {
    MASTER = false;
    RESETS += 1;
    TUNING.fill(0.0);
}
#[no_mangle]
pub unsafe extern "C" fn MTS_RegisterMaster(_: *mut c_void) {
    MASTER = true;
}
#[no_mangle]
pub unsafe extern "C" fn MTS_DeregisterMaster() {
    MASTER = false;
}
#[no_mangle]
pub unsafe extern "C" fn MTS_SetNoteTunings(hz: *const f64) {
    ptr::copy_nonoverlapping(hz, TUNING.as_mut_ptr(), 128);
    WRITES += 1;
}
#[no_mangle]
pub unsafe extern "C" fn MTS_SetScaleName(_: *const i8) {}
#[no_mangle]
pub unsafe extern "C" fn MTS_ClearNoteFilter() {}
#[no_mangle]
pub unsafe extern "C" fn MTS_SetMultiChannel(_: bool, _: i8) {}
#[no_mangle]
pub unsafe extern "C" fn MTS_GetNumClients() -> i32 {
    if RESETS == 0 { 9 } else { 0 }
}
#[no_mangle]
pub unsafe extern "C" fn IntonTestResets() -> i32 {
    RESETS
}
#[no_mangle]
pub unsafe extern "C" fn IntonTestWrites() -> i32 {
    WRITES
}
#[no_mangle]
pub unsafe extern "C" fn MTS_GetTuningTable() -> *const f64 {
    ptr::addr_of!(TUNING).cast::<f64>()
}
