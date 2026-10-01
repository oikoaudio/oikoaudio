//! Per-user files that all instances of a product share, kept outside host
//! projects. Each product has a folder inside the vendor folder of a standard
//! per-user directory: `Oiko Audio/<Product>` on macOS and Windows, and
//! `oikoaudio/<product>` elsewhere.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// The file that holds a product's per-user settings.
pub const PREFERENCES_FILE: &str = "preferences.json";
const UI_SCALE_KEY: &str = "ui_scale";

/// The kind of per-user directory. macOS and Windows use one directory for both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// Settings, such as the preferences file.
    Config,
    /// User content, such as an imported library.
    Data,
}

/// `product`'s folder in the per-user directory for `location`. `product` is
/// the display name without the vendor, such as `"Wow"`.
pub fn product_dir(location: Location, product: &str) -> PathBuf {
    platform_home(location).join(product_relative_dir(product))
}

/// `product`'s folder relative to a per-user base directory.
pub fn product_relative_dir(product: &str) -> PathBuf {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        Path::new("Oiko Audio").join(product)
    } else {
        Path::new("oikoaudio").join(product.to_lowercase())
    }
}

/// The platform's per-user base directory for `location`, without the vendor folder.
#[cfg(target_os = "macos")]
pub fn platform_home(_location: Location) -> PathBuf {
    home_dir().join("Library/Application Support")
}

/// The platform's per-user base directory for `location`, without the vendor folder.
#[cfg(target_os = "windows")]
pub fn platform_home(_location: Location) -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData/Local"))
}

/// The platform's per-user base directory for `location`, without the vendor folder.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn platform_home(location: Location) -> PathBuf {
    let (variable, fallback) = match location {
        Location::Config => ("XDG_CONFIG_HOME", ".config"),
        Location::Data => ("XDG_DATA_HOME", ".local/share"),
    };
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home_dir().join(fallback))
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Replace `path` with `bytes`, creating parent directories. Readers see either
/// the old or the new contents, never a partial write.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Path has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(
        ".oiko-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
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

/// The interface scale last chosen in a product's editor, so that new
/// instances open at it. Host project state takes precedence when restored.
#[derive(Clone, Debug)]
pub struct UiScalePreference {
    path: Option<PathBuf>,
}

impl UiScalePreference {
    /// Stored in `product`'s per-user preferences file.
    pub fn for_product(product: &str) -> Self {
        Self::at(product_dir(Location::Config, product).join(PREFERENCES_FILE))
    }

    /// Stored in the JSON preferences file at `path`.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }

    /// Remembers nothing, for tests that must not touch the user's files.
    pub fn disabled() -> Self {
        Self { path: None }
    }

    /// The remembered scale, snapped to a supported step. `None` when the file
    /// is missing, unreadable or holds no valid scale.
    pub fn load(&self) -> Option<f32> {
        let bytes = fs::read(self.path.as_ref()?).ok()?;
        let preferences: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let scale = preferences.get(UI_SCALE_KEY)?.as_f64()?;
        scale
            .is_finite()
            .then(|| oiko_ui::scale::nearest_scale(scale) as f32)
    }

    /// Remember `scale`, keeping any other settings in the file. Does nothing
    /// when disabled.
    pub fn store(&self, scale: f32) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut preferences = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
        preferences[UI_SCALE_KEY] = oiko_ui::scale::closest_ui_scale(scale).into();
        let bytes = serde_json::to_vec_pretty(&preferences).map_err(|e| e.to_string())?;
        atomic_write(path, &bytes)
    }
}

#[cfg(test)]
mod tests;
