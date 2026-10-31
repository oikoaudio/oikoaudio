//! A read runs in the audio callback, so it must not allocate. The library forbids `unsafe`, so the
//! counting allocator this takes lives here, in a crate of its own.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

use oiko_dsp::fractional_delay::{MinimumPhaseRateSincF32, SampleSourceF32};

thread_local! {
    /// Per thread, as the test harness allocates on its own threads while a test runs.
    static CALLS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    // Fails only while the thread is ending, after the last read.
    let _ = CALLS.try_with(|calls| calls.set(calls.get() + 1));
}

struct Counting;

// SAFETY: every call passes its arguments to `System` unchanged, and counting allocates nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        count();
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The calls to the allocator that `work` makes on this thread.
fn calls_in(work: impl FnOnce()) -> usize {
    let before = CALLS.with(Cell::get);
    work();
    CALLS.with(Cell::get) - before
}

struct Constant;

impl SampleSourceF32 for Constant {
    fn at_f32(&self, _index: i64) -> f32 {
        0.25
    }
}

#[test]
fn the_count_sees_an_allocation() {
    assert!(calls_in(|| drop(black_box(vec![0_u8; 16]))) >= 2);
}

#[test]
fn reads_call_no_allocator_at_any_rate() {
    let reader = MinimumPhaseRateSincF32::new(48, 256, 16.0, 0.87, 9.0);
    let source = Constant;
    let calls = calls_in(|| {
        for rate in [0.25, 1.0, 1.03, 2.0, 16.0, 40.0, f64::NAN] {
            for position in [-3.5, 0.0, 10_000.0, 10_000.37] {
                black_box(reader.sample(&source, position, rate));
                black_box(reader.sample_stereo(&source, &source, position, rate));
                black_box(reader.sample_flat(&source, position, rate));
                black_box(reader.sample_stereo_flat(&source, &source, position, rate));
            }
        }
    });
    assert_eq!(calls, 0);
}
