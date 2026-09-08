use crate::{
    engine::Parameters,
    tuning::{Preset, ValidatedScale, twelve_edo},
};
use serde::{Deserialize, Serialize};
pub const SET_SIZE: usize = 32;
pub const MAX_STATE_BYTES: usize = 72 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub parameters: Parameters,
    pub slots: Vec<Option<Preset>>,
    /// Added slots remain available after their contents are cleared.
    #[serde(default)]
    pub allocated_slots: u32,
    #[serde(default)]
    pub scale_set: bool,
    #[serde(default)]
    pub set_collapsed: bool,
    // Preserve the audible table even if the selected slot is empty or was cleared.
    #[serde(default)]
    pub held_log: Option<Vec<f64>>,
    #[serde(default)]
    pub held_name: Option<String>,
}
impl Default for Project {
    fn default() -> Self {
        let mut slots = vec![None; 32];
        let mut p = twelve_edo();
        p.source_id = Some("factory:12edo".into());
        slots[0] = Some(p);
        Self {
            version: 1,
            parameters: Parameters::default(),
            slots,
            allocated_slots: 1,
            scale_set: false,
            set_collapsed: false,
            held_log: None,
            held_name: None,
        }
    }
}
impl Project {
    pub fn uses_scale_set(&self) -> bool {
        self.scale_set
            || self.allocated_slots & !1 != 0
            || self.slots.iter().skip(1).any(Option::is_some)
    }
    pub fn has_slot(&self, index: usize) -> bool {
        index < SET_SIZE
            && (self.allocated_slots & (1 << index) != 0 || self.slots[index].is_some())
    }
    pub fn assign(&mut self, index: usize, p: Preset) -> Result<(), String> {
        if index >= SET_SIZE {
            return Err("Invalid slot".into());
        }
        self.assign_prepared(index, &ValidatedScale::new(p)?)
    }
    pub(crate) fn assign_prepared(
        &mut self,
        index: usize,
        scale: &ValidatedScale,
    ) -> Result<(), String> {
        if index >= SET_SIZE {
            return Err("Invalid slot".into());
        }
        if index > 0 {
            self.scale_set = true;
        }
        self.slots[index] = Some(scale.preset().clone());
        self.allocated_slots |= 1 << index;
        Ok(())
    }
    pub fn clear(&mut self, index: usize) -> Result<(), String> {
        if index >= SET_SIZE {
            return Err("Invalid slot".into());
        }
        if self.slots[index + 1..].iter().any(Option::is_some) {
            if self.has_slot(index) {
                self.allocated_slots |= 1 << index;
            }
        } else {
            self.allocated_slots &= !(1 << index);
        }
        self.slots[index] = None;
        Ok(())
    }
    pub fn label(&self, index: usize) -> String {
        format!(
            "{}: {}",
            index + 1,
            self.slots
                .get(index)
                .and_then(Option::as_ref)
                .map_or("Empty", |p| &p.display_name)
        )
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| e.to_string())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        Ok(ValidatedProject::decode(bytes)?.project)
    }

    /// Check every invariant required by the tuning engine, including native parsing.
    /// Call this at load boundaries even when a caller constructed the project directly.
    pub fn validate(&self) -> Result<(), String> {
        self.validate_structure()?;
        for s in self.slots.iter().flatten() {
            s.prepare()?;
        }
        Ok(())
    }
    fn validate_structure(&self) -> Result<(), String> {
        let p = self;
        if p.version != 1 || p.slots.len() != SET_SIZE {
            return Err("Unsupported project version or slot count".into());
        }
        if p.parameters != Parameters::from_values(p.parameters.values()) {
            return Err("Invalid project parameters".into());
        }
        if let Some(h) = &p.held_log
            && (h.len() != 128
                || h.iter()
                    .any(|x| !x.is_finite() || !x.exp2().is_finite() || x.exp2() <= 0.0))
        {
            return Err("Invalid retained tuning table".into());
        }
        Ok(())
    }
}

/// Index of the same slot contents after an insertion move (not a swap).
pub fn moved_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if to < from && index >= to && index < from {
        index + 1
    } else {
        index
    }
}

/// Prepared load transaction. The serialized Project remains the public wire format.
#[derive(Clone)]
pub struct ValidatedProject {
    pub(crate) project: Project,
    pub(crate) scales: Vec<Option<ValidatedScale>>,
}
impl ValidatedProject {
    pub fn new(project: Project) -> Result<Self, String> {
        project.validate_structure()?;
        let scales = project
            .slots
            .iter()
            .map(|s| s.clone().map(ValidatedScale::new).transpose())
            .collect::<Result<_, _>>()?;
        Ok(Self { project, scales })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_STATE_BYTES {
            return Err("Project state is too large".into());
        }
        let project: Project = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let mut validated = Self::new(project)?;
        for n in 0..SET_SIZE {
            if validated.project.slots[n].is_some() {
                validated.project.allocated_slots |= 1 << n;
            }
        }
        validated.project.scale_set = validated.project.uses_scale_set();
        Ok(validated)
    }
    pub fn set_parameters(&mut self, parameters: Parameters) {
        self.project.parameters = Parameters::from_values(parameters.values());
    }
}
