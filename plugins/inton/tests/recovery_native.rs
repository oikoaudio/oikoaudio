//! Exercise the real control worker and dynamic ABI against a private stale-state fake.
use inton_core::{mts::MasterStatus, runtime::Shared};
use std::{
    sync::{Arc, atomic::Ordering::SeqCst},
    time::{Duration, Instant},
};
#[test]
fn native_recovery_reconnects_and_publishes_current_project() {
    if std::env::var_os("INTON_RECOVERY_TEST_CHILD").is_none() {
        let dir = std::env::temp_dir().join(format!("inton-recovery-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(target_os = "linux")]
        let dll = dir.join("fake-mts.so");
        #[cfg(target_os = "macos")]
        let dll = dir.join("fake-mts.dylib");
        #[cfg(target_os = "windows")]
        let dll = dir.join("fake-mts.dll");
        assert!(
            std::process::Command::new("rustc")
                .args([
                    "--crate-type",
                    "cdylib",
                    "tests/fixtures/mts_stale.rs",
                    "-o"
                ])
                .arg(&dll)
                .status()
                .unwrap()
                .success()
        );
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native_recovery_reconnects_and_publishes_current_project",
                "--nocapture",
            ])
            .env("INTON_RECOVERY_TEST_CHILD", "1")
            .env("INTON_MTS_LIBRARY", &dll)
            .output()
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    unsafe {
        let lib = libloading::Library::new(std::env::var_os("INTON_MTS_LIBRARY").unwrap()).unwrap();
        // Require our fake marker before creating a worker; never reset a real library.
        let resets: libloading::Symbol<unsafe extern "C" fn() -> i32> =
            lib.get(b"IntonTestResets\0").unwrap();
        let writes: libloading::Symbol<unsafe extern "C" fn() -> i32> =
            lib.get(b"IntonTestWrites\0").unwrap();
        let table: libloading::Symbol<unsafe extern "C" fn() -> *const f64> =
            lib.get(b"MTS_GetTuningTable\0").unwrap();
        let shared = Arc::new(Shared::new());
        shared
            .assign(
                0,
                inton::library::Library::factory()
                    .load("factory:22edo")
                    .unwrap(),
            )
            .unwrap();
        let before = shared.snapshot().encode().unwrap();
        let expected = shared.publication(0.).0;
        let worker = shared.start().unwrap();
        let wait = |status| {
            let start = Instant::now();
            while shared.status.lock().unwrap().0 != status {
                assert!(start.elapsed() < Duration::from_secs(2));
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        wait(MasterStatus::BlockedByOtherMaster);
        assert!(shared.recovery_available.load(SeqCst));
        assert_eq!(resets(), 0);
        assert_eq!(writes(), 0);
        shared.recovery_requested.store(true, SeqCst);
        wait(MasterStatus::ActiveMaster);
        assert_eq!(resets(), 1);
        assert_eq!(writes(), 1);
        assert_eq!(std::slice::from_raw_parts(table(), 128), expected);
        assert_eq!(shared.snapshot().encode().unwrap(), before);
        shared.stop.store(true, SeqCst);
        worker.join().unwrap();
    }
}
