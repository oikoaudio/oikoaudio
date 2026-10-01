use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oiko-user-storage-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn product_folders_sit_in_the_vendor_folder() {
    let expected = if cfg!(any(target_os = "macos", target_os = "windows")) {
        Path::new("Oiko Audio").join("Wow")
    } else {
        Path::new("oikoaudio").join("wow")
    };
    assert_eq!(product_relative_dir("Wow"), expected);
    assert!(product_dir(Location::Config, "Wow").ends_with(&expected));
}

#[test]
fn ui_scale_round_trips_snapped_and_keeps_other_settings() {
    let dir = temp_dir("round-trip");
    let path = dir.join("nested").join(PREFERENCES_FILE);
    let preference = UiScalePreference::at(&path);
    assert_eq!(preference.load(), None);

    preference.store(1.3).unwrap();
    assert_eq!(preference.load(), Some(1.25));

    fs::write(&path, br#"{"theme":"light","ui_scale":2.0}"#).unwrap();
    preference.store(0.75).unwrap();
    assert_eq!(preference.load(), Some(0.75));
    let stored: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored["theme"], "light");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unusable_files_are_ignored_and_replaced() {
    let dir = temp_dir("unusable");
    let path = dir.join(PREFERENCES_FILE);
    let preference = UiScalePreference::at(&path);
    for contents in [&b"not json"[..], br#"{"ui_scale":"big"}"#, b"[1.5]"] {
        fs::write(&path, contents).unwrap();
        assert_eq!(preference.load(), None);
    }
    preference.store(1.5).unwrap();
    assert_eq!(preference.load(), Some(1.5));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn disabled_preference_never_touches_files() {
    let preference = UiScalePreference::disabled();
    assert_eq!(preference.store(1.5), Ok(()));
    assert_eq!(preference.load(), None);
}
