use inton_core::tuning::{MAX_TUNING_BYTES, Preset, ValidatedScale};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::SystemTime,
};
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub path: Option<PathBuf>,
    pub kbm: Option<PathBuf>,
    stamp: Vec<(u64, Option<SystemTime>)>,
    preset: Option<Preset>,
}
#[derive(Default)]
pub struct Library {
    pub entries: Vec<Entry>,
    cache: Mutex<HashMap<String, Result<ValidatedScale, String>>>,
    parses: AtomicUsize,
}
impl Clone for Library {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            cache: Mutex::new(self.cache.lock().unwrap().clone()),
            parses: AtomicUsize::new(self.parses.load(Ordering::Relaxed)),
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Preferences {
    #[serde(default)]
    pub view: crate::editor_model::ViewPreferences,
    pub folder: PathBuf,
    pub favorites: BTreeSet<String>,
}
#[derive(Deserialize)]
struct FactoryRecord {
    #[serde(flatten)]
    preset: Preset,
    favorite: bool,
}
fn records() -> Vec<FactoryRecord> {
    serde_json::from_str(include_str!("../resources/library.json"))
        .expect("validated factory metadata")
}
pub fn data_home() -> PathBuf {
    platform_data_home().join("oiko/inton")
}
#[cfg(all(not(test), target_os = "linux"))]
fn platform_data_home() -> PathBuf {
    xdg_home("XDG_DATA_HOME", ".local/share")
}
#[cfg(all(not(test), target_os = "macos"))]
fn platform_data_home() -> PathBuf {
    home_dir().join("Library/Application Support")
}
#[cfg(all(not(test), target_os = "windows"))]
fn platform_data_home() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData/Local"))
}
#[cfg(not(test))]
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
#[cfg(all(not(test), target_os = "linux"))]
fn xdg_home(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home_dir().join(fallback))
}
pub fn preferences_path() -> PathBuf {
    platform_config_home().join("oiko/inton/preferences.json")
}
#[cfg(all(not(test), target_os = "linux"))]
fn platform_config_home() -> PathBuf {
    xdg_home("XDG_CONFIG_HOME", ".config")
}
#[cfg(all(not(test), target_os = "macos"))]
fn platform_config_home() -> PathBuf {
    home_dir().join("Library/Application Support")
}
#[cfg(all(not(test), target_os = "windows"))]
fn platform_config_home() -> PathBuf {
    platform_data_home()
}
// Unit tests exercise real preference writes, but each test thread owns its
// storage so UI tests cannot change another test's favorites, zoom or library.
#[cfg(test)]
fn test_storage_home() -> PathBuf {
    struct Storage(PathBuf);
    impl Drop for Storage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    thread_local! {
        static STORAGE: Storage = {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            Storage(std::env::temp_dir().join(format!(
                "inton-unit-{}-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            )))
        };
    }
    STORAGE.with(|storage| storage.0.clone())
}
#[cfg(test)]
fn platform_data_home() -> PathBuf {
    test_storage_home().join("data")
}
#[cfg(test)]
fn platform_config_home() -> PathBuf {
    test_storage_home().join("config")
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            view: crate::editor_model::ViewPreferences::default(),
            folder: data_home().join("scales"),
            favorites: records()
                .into_iter()
                .filter(|r| r.favorite)
                .map(|r| r.preset.source_id.unwrap())
                .collect(),
        }
    }
}
impl Preferences {
    pub fn load_from(path: &Path) -> Result<Self, String> {
        serde_json::from_slice::<Self>(&fs::read(path).map_err(|e| e.to_string())?)
            .map(|mut prefs| {
                prefs.view.scale =
                    crate::editor_model::ViewPreferences::nearest_scale(prefs.view.scale);
                prefs
            })
            .map_err(|e| e.to_string())
    }
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        atomic_write(
            path,
            &serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Path has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(
        ".inton-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })()
    .map_err(|e: std::io::Error| e.to_string());
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
fn read_text(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .take((MAX_TUNING_BYTES + 1) as u64);
    let mut text = String::new();
    f.read_to_string(&mut text).map_err(|e| e.to_string())?;
    if text.len() > MAX_TUNING_BYTES {
        return Err("Tuning file exceeds 1 MiB".into());
    }
    Ok(text)
}
impl Library {
    pub fn factory() -> Self {
        Self {
            entries: records()
                .into_iter()
                .map(|r| {
                    let p = r.preset;
                    Entry {
                        id: p.source_id.clone().unwrap(),
                        name: p.display_name.clone(),
                        category: format!("Factory/{}", p.category),
                        description: p.description.clone(),
                        tags: p.tags.clone(),
                        aliases: p.aliases.clone(),
                        path: None,
                        kbm: None,
                        stamp: vec![],
                        preset: Some(p),
                    }
                })
                .collect(),
            ..Default::default()
        }
    }
    pub fn parse_count(&self) -> usize {
        self.parses.load(Ordering::Relaxed)
    }
    pub fn user_count(&self) -> usize {
        self.entries.iter().filter(|e| e.path.is_some()).count()
    }
    pub fn search(&self, query: &str) -> Vec<&Entry> {
        let cached = self.cache.lock().unwrap();
        let tokens: Vec<_> = query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        self.entries
            .iter()
            .filter(|e| {
                let text = format!(
                    "{} {} {} {} {} {} {}",
                    cached
                        .get(&e.id)
                        .and_then(|p| p.as_ref().ok())
                        .map(|p| p.preset().description.as_str())
                        .unwrap_or(""),
                    e.name,
                    e.category,
                    e.description,
                    e.tags.join(" "),
                    e.aliases.join(" "),
                    e.path
                        .as_ref()
                        .map(|p| p.to_string_lossy())
                        .unwrap_or_default()
                )
                .to_lowercase();
                tokens.iter().all(|t| text.contains(t))
            })
            .collect()
    }
    pub fn rescan(&mut self, root: &Path) -> Result<(), String> {
        let root = fs::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
        let mut files = vec![];
        let mut pending = vec![root.clone()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let ty = entry.file_type().map_err(|e| e.to_string())?;
                let path = entry.path();
                if ty.is_dir() {
                    pending.push(path)
                } else if ty.is_file() {
                    files.push(path)
                }
            }
        }
        files.sort();
        let mut kbms = HashMap::new();
        for path in &files {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("kbm"))
            {
                kbms.entry(path.with_extension(""))
                    .or_insert_with(|| path.clone());
            }
        }
        let mut next: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.path.is_none())
            .cloned()
            .collect();
        for path in files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("scl")))
        {
            let kbm = kbms.get(&path.with_extension("")).cloned();
            let mut stamp = vec![];
            for p in std::iter::once(path).chain(kbm.iter()) {
                let m = fs::metadata(p).map_err(|e| e.to_string())?;
                stamp.push((m.len(), m.modified().ok()));
            }
            let relative = path.strip_prefix(&root).unwrap();
            let folder = relative.parent().unwrap_or(Path::new(""));
            next.push(Entry {
                id: format!("user:{}", path.display()),
                name: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                category: if folder.as_os_str().is_empty() {
                    "User".into()
                } else {
                    format!("User/{}", folder.display())
                },
                description: String::new(),
                tags: vec![],
                aliases: vec![],
                path: Some(path.clone()),
                kbm,
                stamp,
                preset: None,
            });
        }
        let mut cache = self.cache.lock().unwrap();
        cache.retain(|id, _| {
            next.iter().find(|e| &e.id == id).is_some_and(|n| {
                self.entries
                    .iter()
                    .find(|e| &e.id == id)
                    .is_some_and(|o| o.stamp == n.stamp && o.kbm == n.kbm)
            })
        });
        drop(cache);
        self.entries = next;
        Ok(())
    }
    pub fn load(&self, id: &str) -> Result<Preset, String> {
        Ok(self.load_prepared(id)?.preset().clone())
    }
    pub fn load_prepared(&self, id: &str) -> Result<ValidatedScale, String> {
        let e = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or("Scale is no longer in this library; rescan")?;
        if let Some(p) = self.cache.lock().unwrap().get(id) {
            return p.clone();
        }
        if let Some(p) = &e.preset {
            let result = ValidatedScale::new(p.clone());
            self.cache.lock().unwrap().insert(id.into(), result.clone());
            return result;
        }
        self.parses.fetch_add(1, Ordering::Relaxed);
        let result = (|| {
            let mut p = Preset::new(
                e.name.clone(),
                read_text(e.path.as_ref().unwrap())?,
                e.kbm.as_ref().map(|p| read_text(p)).transpose()?,
            );
            p.description = p
                .scl_text
                .lines()
                .find(|line| !line.trim_start().starts_with('!'))
                .unwrap_or("")
                .trim()
                .into();
            p.source_id = Some(e.id.clone());
            p.category = e.category.clone();
            p.source = e.path.as_ref().unwrap().display().to_string();
            ValidatedScale::new(p)
        })();
        self.cache.lock().unwrap().insert(id.into(), result.clone());
        result
    }
    pub fn cached_error(&self, id: &str) -> Option<String> {
        self.cache
            .lock()
            .unwrap()
            .get(id)
            .and_then(|p| p.as_ref().err().cloned())
    }
    pub fn import(&mut self, source: &Path, folder: &Path) -> Result<String, String> {
        if !source
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("scl"))
        {
            return Err("Choose a .scl file".into());
        }
        fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        let source = fs::canonicalize(source).map_err(|e| e.to_string())?;
        let folder = fs::canonicalize(folder).map_err(|e| e.to_string())?;
        let kbm = source.with_extension("kbm");
        let kbm_upper = source.with_extension("KBM");
        let kbm = if kbm.exists() {
            Some(kbm)
        } else if kbm_upper.exists() {
            Some(kbm_upper)
        } else {
            None
        };
        let p = Preset::new(
            source.file_stem().unwrap().to_string_lossy().into(),
            read_text(&source)?,
            kbm.as_ref().map(|p| read_text(p)).transpose()?,
        );
        p.prepare()?;
        let mut dest = folder.join(source.file_name().unwrap());
        if source != dest {
            let base = source.file_stem().unwrap().to_string_lossy();
            let mut i = 1;
            while dest.exists() || dest.with_extension("kbm").exists() {
                dest = folder.join(format!("{base}-{i}.scl"));
                i += 1;
            }
            // create_new avoids overwriting another writer's file.
            use std::io::Write;
            let mut out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&dest)
                .map_err(|e| e.to_string())?;
            out.write_all(p.scl_text.as_bytes())
                .map_err(|e| e.to_string())?;
            if let Some(k) = p.kbm_text {
                let result = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(dest.with_extension("kbm"))
                    .and_then(|mut f| f.write_all(k.as_bytes()));
                if let Err(e) = result {
                    let _ = fs::remove_file(&dest);
                    return Err(e.to_string());
                }
            }
        }
        self.rescan(&folder)?;
        Ok(format!("user:{}", dest.display()))
    }
}
