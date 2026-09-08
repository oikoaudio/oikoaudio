use super::*;
#[test]
fn snapshot_preserves_tuning_and_rejects_in_progress_writes() {
    let snapshot = Snapshot::default();
    assert!(!snapshot.read().unwrap().0.active);
    let mut t = Tuning {
        active: true,
        map_size: Some(22),
        map_start: 60,
        ..Tuning::default()
    };
    t.frequencies[60] = 222.25;
    t.mapped[61] = false;
    let mut name = [0; 256];
    name[..6].copy_from_slice(b"22 EDO");
    snapshot.publish(&t, &name);
    let (received, name) = snapshot.read().unwrap();
    assert_eq!(received.frequencies, t.frequencies);
    assert_eq!(received.mapped, t.mapped);
    assert_eq!((received.map_size, received.map_start), (Some(22), 60));
    assert_eq!(name, "22 EDO");
    snapshot.generation.fetch_add(1, SeqCst);
    assert!(snapshot.read().is_none());
}
#[test]
#[cfg(unix)]
#[ignore = "requires private counting backend via WEFT_MTS_LIBRARY"]
fn editor_does_not_register_an_extra_client() {
    unsafe extern "C" {
        fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    let path = std::ffi::CString::new(std::env::var("WEFT_MTS_LIBRARY").unwrap()).unwrap();
    let handle = unsafe { dlopen(path.as_ptr(), 2) };
    assert!(!handle.is_null());
    let symbol = unsafe { dlsym(handle, c"MTS_GetNumClients".as_ptr()) };
    assert!(!symbol.is_null());
    let count: unsafe extern "C" fn() -> i32 = unsafe { std::mem::transmute(symbol) };
    let baseline = unsafe { count() };
    let plugin = crate::SpectralPlugin::default();
    assert_eq!(unsafe { count() }, baseline + 1);
    let editor =
        crate::editor::SpectralEditor::new(plugin.params.clone(), plugin.analysis_display.clone());
    assert_eq!(unsafe { count() }, baseline + 1);
    drop(editor);
    assert_eq!(unsafe { count() }, baseline + 1);
    drop(plugin);
    assert_eq!(unsafe { count() }, baseline);
}
#[test]
fn inactive_is_exactly_neutral() {
    let t = Tuning::default();
    assert!(t.cells().is_none());
    for n in 0..128 {
        assert_eq!(t.offset(n), 0.0);
    }
}
#[test]
fn nineteen_edo_cells_align_and_repeat() {
    let t = Tuning {
        active: true,
        frequencies: std::array::from_fn(|n| 440.0 * 2.0_f32.powf((n as f32 - 69.0) / 19.0)),
        map_size: Some(19),
        map_start: 69,
        ..Tuning::default()
    };
    let cells = t.cells().unwrap();
    assert!(t.boundary(69) && t.boundary(88) && !t.boundary(70));
    assert!(((cells[69].1 * cells[69].2).sqrt() - 440.0).abs() < 0.001);
    assert!((cells[70].1 / cells[69].1 - 2.0_f32.powf(1.0 / 19.0)).abs() < 0.00001);
    assert!((t.offset(88) + 7.0).abs() < 0.0001);
}
#[test]
fn filtered_notes_are_not_selectable_and_bad_order_falls_back() {
    let mut t = Tuning {
        active: true,
        ..Tuning::default()
    };
    t.mapped[60] = false;
    assert!(!t.cells().unwrap().iter().any(|c| c.0 == 60));
    t.frequencies[61] = t.frequencies[62];
    assert!(t.cells().is_none());
}
#[test]
fn non_octave_tuning_needs_no_equal_division_assumption() {
    let t = Tuning {
        active: true,
        frequencies: std::array::from_fn(|n| 220.0 * 3.0_f32.powf((n as f32 - 60.0) / 13.0)),
        ..Tuning::default()
    };
    assert!((t.frequencies[73] / t.frequencies[60] - 3.0).abs() < 0.00001);
    assert!(t.cells().is_some());
    assert!(!t.boundary(60));
}
