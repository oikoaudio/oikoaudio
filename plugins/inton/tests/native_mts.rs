use inton::library::Library;
use inton_core::mts::{Master, MasterApi, MasterStatus, Native};
#[test]
#[ignore = "Run with INTON_MTS_LIBRARY set to the supplied official library; requires no other MTS master"]
fn official_library_publishes_all_notes_and_releases() {
    unsafe {
        let path = std::env::var_os("INTON_MTS_LIBRARY")
            .expect("Set INTON_MTS_LIBRARY to vendor/mts-esp/libMTS/Linux/x86_64/libMTS.so");
        let lib = libloading::Library::new(path).unwrap();
        let has: libloading::Symbol<unsafe extern "C" fn() -> bool> =
            lib.get(b"MTS_HasMaster\0").unwrap();
        let table: libloading::Symbol<unsafe extern "C" fn() -> *const f64> =
            lib.get(b"MTS_GetTuningTable\0").unwrap();
        let register: libloading::Symbol<unsafe extern "C" fn()> =
            lib.get(b"MTS_RegisterClient\0").unwrap();
        let deregister: libloading::Symbol<unsafe extern "C" fn()> =
            lib.get(b"MTS_DeregisterClient\0").unwrap();
        let api = Native::load();
        assert!(api.available());
        assert!(
            !has(),
            "Another master exists; do not reset or override it for this test"
        );
        register();
        let mut master = Master::new(api);
        let library = Library::factory();
        let start = std::time::Instant::now();
        for entry in &library.entries {
            let t = library.load(&entry.id).unwrap().prepare().unwrap();
            assert_eq!(
                master.update(true, &t.hz, &entry.name, t.period),
                MasterStatus::ActiveMaster
            );
            assert!(has());
            assert!(master.clients() >= 1);
            let actual = std::slice::from_raw_parts(table(), 128);
            for (actual, expected) in actual.iter().zip(t.hz) {
                assert!((actual / expected - 1.).abs() < 1e-12)
            }
        }
        eprintln!(
            "Official MTS library: 36 complete tuning publications in {:?}",
            start.elapsed()
        );
        let mut blocked = Master::new(Native::load());
        assert_eq!(
            blocked.update(true, &[440.; 128], "blocked", 2.),
            MasterStatus::BlockedByOtherMaster
        );
        drop(blocked);
        assert!(has());
        drop(master);
        assert!(!has());
        deregister();
    }
}
