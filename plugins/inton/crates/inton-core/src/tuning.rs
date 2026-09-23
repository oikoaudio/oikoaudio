pub use oiko_tuning::{MAX_TUNING_BYTES, Prepared};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub source_id: Option<String>,
    pub display_name: String,
    pub scl_text: String,
    pub kbm_text: Option<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub attribution: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}
impl Preset {
    pub fn new(display_name: String, scl_text: String, kbm_text: Option<String>) -> Self {
        Self {
            source_id: None,
            display_name,
            scl_text,
            kbm_text,
            category: String::new(),
            description: String::new(),
            source: String::new(),
            attribution: String::new(),
            tags: vec![],
            aliases: vec![],
        }
    }
    pub fn prepare(&self) -> Result<Prepared, String> {
        oiko_tuning::prepare(&self.scl_text, self.kbm_text.as_deref())
    }
}

pub fn twelve_edo() -> Preset {
    Preset::new(
        "12 EDO".into(),
        include_str!("../resources/12edo.scl").into(),
        None,
    )
}

/// An immutable source scale and its validated tuning. Dropping the last clone frees
/// the scale, so keep clones off the audio thread.
#[derive(Clone, Debug)]
pub struct ValidatedScale(std::sync::Arc<(Preset, Prepared)>);
impl ValidatedScale {
    pub fn new(preset: Preset) -> Result<Self, String> {
        let tuning = preset.prepare()?;
        Ok(Self(std::sync::Arc::new((preset, tuning))))
    }
    pub fn preset(&self) -> &Preset {
        &self.0.0
    }
    pub fn tuning(&self) -> &Prepared {
        &self.0.1
    }
    pub fn parts(&self) -> &(Preset, Prepared) {
        &self.0
    }
}

pub(crate) fn default_scale() -> ValidatedScale {
    static SCALE: std::sync::OnceLock<ValidatedScale> = std::sync::OnceLock::new();
    SCALE
        .get_or_init(|| ValidatedScale::new(twelve_edo()).expect("factory validated"))
        .clone()
}
