use inton::library::{Library, Preferences};
use inton_core::tuning::twelve_edo;
use std::{fs, path::PathBuf};
fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("inton-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn curated_factory_valid_searchable_and_favorited() {
    let lib = Library::factory();
    assert!((30..=60).contains(&lib.entries.len()));
    for e in &lib.entries {
        lib.load(&e.id).unwrap().prepare().unwrap();
    }
    assert!(!lib.search("tritave").is_empty());
    let expected: std::collections::BTreeSet<String> = [
        "detune-soft-circuit",
        "quarter-comma",
        "19edo",
        "22edo",
        "31edo",
        "7edo",
        "13ed3",
    ]
    .into_iter()
    .map(|id| format!("factory:{id}"))
    .collect();
    assert_eq!(Preferences::default().favorites, expected);
}
#[test]
fn large_scan_is_lazy_pairing_categories_cache_and_errors() {
    let dir = temp("scan");
    fs::create_dir_all(dir.join("Sevish/EDO")).unwrap();
    for i in 0..1700 {
        fs::write(
            dir.join(format!("Sevish/EDO/scale-{i}.scl")),
            "invalid on purpose",
        )
        .unwrap();
    }
    fs::write(dir.join("paired.scl"), twelve_edo().scl_text).unwrap();
    fs::write(dir.join("paired.kbm"), "0\n0\n127\n60\n69\n440\n12\n").unwrap();
    let mut lib = Library::factory();
    lib.rescan(&dir).unwrap();
    assert_eq!(lib.user_count(), 1701);
    assert_eq!(lib.parse_count(), 0);
    assert_eq!(lib.search("Sevish").len(), 1700);
    let id = lib.search("paired")[0].id.clone();
    assert!(lib.load(&id).unwrap().kbm_text.is_some());
    assert_eq!(lib.parse_count(), 1);
    lib.rescan(&dir).unwrap();
    lib.load(&id).unwrap();
    assert_eq!(lib.parse_count(), 1);
    fs::write(dir.join("paired.kbm"), "broken!").unwrap();
    lib.rescan(&dir).unwrap();
    assert!(lib.load(&id).is_err());
    fs::remove_file(dir.join("paired.scl")).unwrap();
    lib.rescan(&dir).unwrap();
    assert!(lib.search("paired").is_empty());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn preferences_roundtrip_and_import_owned_copy() {
    let dir = temp("prefs");
    let from = temp("import");
    fs::write(from.join("scale.scl"), twelve_edo().scl_text).unwrap();
    let mut prefs = Preferences {
        folder: dir.join("scales"),
        ..Default::default()
    };
    prefs.favorites.insert("user:test".into());
    prefs.save_to(&dir.join("prefs.json")).unwrap();
    assert!(
        Preferences::load_from(&dir.join("prefs.json"))
            .unwrap()
            .favorites
            .contains("user:test")
    );
    let mut lib = Library::factory();
    let id = lib.import(&from.join("scale.scl"), &prefs.folder).unwrap();
    fs::remove_dir_all(from).unwrap();
    assert!(lib.load(&id).is_ok());
    assert!(
        lib.import(&prefs.folder.join("scale.scl"), &prefs.folder)
            .is_ok()
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn equal_division_categories_keep_the_same_hierarchy_for_every_period() {
    let lib = Library::factory();
    assert_eq!(
        lib.load("factory:12edo").unwrap().category,
        "Equal Divisions/Octave"
    );
    for id in ["factory:13ed3", "factory:13ed8", "factory:12ed4"] {
        let preset = lib.load(id).unwrap();
        assert_eq!(preset.category, "Equal Divisions/Non-octave");
    }
    assert!(
        lib.entries
            .iter()
            .all(|e| !e.category.starts_with("Factory/Non-Octave/Equal"))
    );
}

#[test]
fn loaded_user_descriptions_are_preserved_and_searchable_without_eager_parsing() {
    let folder = temp("description");
    std::fs::write(
        folder.join("example.scl"),
        "! comment\nCopper harmony\n2\n700.0\n2/1\n",
    )
    .unwrap();
    let mut lib = Library::factory();
    lib.rescan(&folder).unwrap();
    assert!(lib.search("copper").is_empty());
    let id = lib
        .entries
        .iter()
        .find(|e| e.path.is_some())
        .unwrap()
        .id
        .clone();
    assert_eq!(lib.load(&id).unwrap().description, "Copper harmony");
    assert_eq!(lib.search("copper").len(), 1);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn legacy_storage_moves_into_the_vendor_folder() {
    use inton::library::migrate_legacy_storage;
    use oiko_plugin::user_storage::{PREFERENCES_FILE, product_relative_dir};
    let relative = product_relative_dir("Inton");
    for shared_home in [false, true] {
        let root = temp(&format!("legacy-{shared_home}"));
        let config = root.join("config");
        let data = if shared_home {
            config.clone()
        } else {
            root.join("data")
        };
        let legacy_scales = data.join("oiko/inton/scales");
        fs::create_dir_all(&legacy_scales).unwrap();
        fs::write(legacy_scales.join("mine.scl"), twelve_edo().scl_text).unwrap();
        let mut prefs = Preferences {
            folder: legacy_scales.clone(),
            ..Default::default()
        };
        prefs.favorites.insert("user:mine".into());
        prefs
            .favorites
            .insert(format!("user:{}", legacy_scales.join("mine.scl").display()));
        prefs
            .save_to(&config.join("oiko/inton").join(PREFERENCES_FILE))
            .unwrap();

        migrate_legacy_storage(&config, &data);

        let moved = Preferences::load_from(&config.join(&relative).join(PREFERENCES_FILE)).unwrap();
        let scales = data.join(&relative).join("scales");
        assert!(moved.favorites.contains("user:mine"));
        assert!(
            moved
                .favorites
                .contains(&format!("user:{}", scales.join("mine.scl").display()))
        );
        assert_eq!(
            moved.favorites.len(),
            Preferences::default().favorites.len() + 2
        );
        assert_eq!(moved.folder, scales);
        assert!(moved.folder.join("mine.scl").is_file());
        assert!(!config.join("oiko").exists());
        assert!(!data.join("oiko").exists());
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn legacy_storage_never_replaces_the_current_folder_or_a_custom_library() {
    use inton::library::migrate_legacy_storage;
    use oiko_plugin::user_storage::{PREFERENCES_FILE, product_relative_dir};
    let relative = product_relative_dir("Inton");
    let home = temp("legacy-existing");
    fs::create_dir_all(home.join("oiko/inton")).unwrap();
    fs::create_dir_all(home.join(&relative)).unwrap();
    migrate_legacy_storage(&home, &home);
    assert!(home.join("oiko/inton").is_dir());
    fs::remove_dir_all(home.join(&relative)).unwrap();

    let custom = home.join("elsewhere");
    Preferences {
        folder: custom.clone(),
        ..Default::default()
    }
    .save_to(&home.join("oiko/inton").join(PREFERENCES_FILE))
    .unwrap();
    migrate_legacy_storage(&home, &home);
    let moved = Preferences::load_from(&home.join(&relative).join(PREFERENCES_FILE)).unwrap();
    assert_eq!(moved.folder, custom);
    fs::remove_dir_all(home).unwrap();
}
