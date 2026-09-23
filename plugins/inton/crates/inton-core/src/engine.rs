use crate::{
    state::{Project, ValidatedProject},
    tuning::{ValidatedScale, default_scale},
};
use serde::{Deserialize, Serialize};
/// An automatable parameter, independent of host parameter IDs and serialized field names.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Parameter {
    Position,
    MorphTime,
    Reference,
    Transpose,
    Enabled,
}
impl Parameter {
    pub const ALL: [Self; 5] = [
        Self::Position,
        Self::MorphTime,
        Self::Reference,
        Self::Transpose,
        Self::Enabled,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub const fn index(self) -> usize {
        match self {
            Self::Position => 0,
            Self::MorphTime => 1,
            Self::Reference => 2,
            Self::Transpose => 3,
            Self::Enabled => 4,
        }
    }
}

/// Values of every automatable parameter. `Parameters::set` clamps each to its range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    /// Selected slot, 0–31.
    pub position: usize,
    /// Morph Time between slots in milliseconds, 0–10000.
    pub morph_ms: f64,
    /// Reference pitch in Hz, 400–480. The whole table is scaled by `reference / 440`.
    pub reference: f64,
    /// Transpose in semitones, −24 to 24.
    pub transpose: i32,
    /// Whether the tuning is published over MTS.
    pub enabled: bool,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            position: 0,
            morph_ms: 0.0,
            reference: 440.0,
            transpose: 0,
            enabled: true,
        }
    }
}
impl Parameters {
    pub fn values(self) -> [f64; Parameter::COUNT] {
        [
            self.position as f64,
            self.morph_ms,
            self.reference,
            self.transpose as f64,
            self.enabled as u8 as f64,
        ]
    }
    pub fn from_values(v: [f64; Parameter::COUNT]) -> Self {
        let mut p = Self::default();
        for (id, v) in Parameter::ALL.into_iter().zip(v) {
            p.set(id, v)
        }
        p
    }
    pub fn set(&mut self, id: Parameter, v: f64) {
        if !v.is_finite() {
            return;
        }
        match id {
            Parameter::Position => self.position = v.trunc().clamp(0.0, 31.0) as usize,
            Parameter::MorphTime => self.morph_ms = v.clamp(0.0, 10000.0),
            Parameter::Reference => self.reference = v.clamp(400.0, 480.0),
            Parameter::Transpose => self.transpose = v.trunc().clamp(-24.0, 24.0) as i32,
            Parameter::Enabled => self.enabled = v.trunc().clamp(0.0, 1.0) != 0.0,
        }
    }
    pub fn value(self, id: Parameter) -> f64 {
        self.values()[id.index()]
    }
    pub fn offset(self) -> f64 {
        (self.reference / 440.0).log2() + self.transpose as f64 / 12.0
    }
}
/// The tuning table for the selected slot, morphing from the previous table over Morph Time.
pub struct Engine {
    pub parameters: Parameters,
    pub empty: bool,
    pub name: String,
    pub period: f64,
    current: [f64; 128],
    source: [f64; 128],
    target: [f64; 128],
    elapsed: f64,
    duration: f64,
    prepared: Vec<Option<ValidatedScale>>,
}
impl Engine {
    pub fn new(p: &Project) -> Result<Self, String> {
        Ok(Self::from_validated(&ValidatedProject::new(p.clone())?))
    }
    pub(crate) fn from_validated(validated: &ValidatedProject) -> Self {
        let p = &validated.project;
        let prepared = validated.scales.clone();
        let selected = prepared[p.parameters.position]
            .as_ref()
            .map(ValidatedScale::tuning);
        let fallback = default_scale();
        let current = if let Some(selected) = selected {
            selected.hz.map(|v| v.log2() + p.parameters.offset())
        } else {
            p.held_log
                .as_ref()
                .and_then(|v| v.as_slice().try_into().ok())
                .unwrap_or(
                    fallback
                        .tuning()
                        .hz
                        .map(|v| v.log2() + p.parameters.offset()),
                )
        };
        let name = p.slots[p.parameters.position]
            .as_ref()
            .map(|s| s.display_name.clone())
            .or(p.held_name.clone())
            .unwrap_or("12 EDO".into());
        let period = selected.map_or(2.0, |s| s.period);
        Self {
            parameters: p.parameters,
            empty: selected.is_none(),
            name,
            period,
            current,
            source: current,
            target: current,
            elapsed: 0.0,
            duration: 0.0,
            prepared,
        }
    }
    pub fn scale(&self, index: usize) -> Option<ValidatedScale> {
        self.prepared.get(index).cloned().flatten()
    }
    pub(crate) fn replace_scale(&mut self, index: usize, scale: Option<ValidatedScale>) {
        self.prepared[index] = scale;
    }
    /// Move already prepared slot contents without restarting the current morph.
    pub(crate) fn move_slot(&mut self, from: usize, to: usize) {
        if from < to {
            self.prepared[from..=to].rotate_left(1);
        } else {
            self.prepared[to..=from].rotate_right(1);
        }
        self.parameters.position = crate::state::moved_index(self.parameters.position, from, to);
    }
    /// Apply new parameter values. Selecting a different slot, or passing `slot_replaced`
    /// when the selected slot's contents changed, starts a morph to that slot's table.
    /// Selecting an empty slot keeps the current table.
    pub fn update(&mut self, parameters: Parameters, p: &Project, slot_replaced: bool) {
        let changed = parameters.position != self.parameters.position || slot_replaced;
        let offset_delta = parameters.offset() - self.parameters.offset();
        // Reference and Transpose changes apply immediately, bypassing Morph Time.
        if offset_delta != 0.0 {
            for table in [&mut self.current, &mut self.source, &mut self.target] {
                for v in table {
                    *v += offset_delta
                }
            }
        }
        self.parameters = parameters;
        if changed {
            self.source = self.current;
            self.elapsed = 0.0;
            if let Some(scale) = &self.prepared[parameters.position] {
                let t = scale.tuning();
                self.empty = false;
                self.name = p.slots[parameters.position]
                    .as_ref()
                    .unwrap()
                    .display_name
                    .clone();
                self.period = t.period;
                self.target = t.hz.map(|v| v.log2() + parameters.offset());
                self.duration = parameters.morph_ms / 1000.0;
                if self.duration == 0.0 {
                    self.current = self.target;
                }
            } else {
                self.empty = true;
                self.target = self.current;
                self.duration = 0.0;
            }
        }
    }
    /// Advance any morph in progress by `seconds`.
    pub fn advance(&mut self, seconds: f64) {
        if self.duration <= 0.0 {
            return;
        }
        self.elapsed = (self.elapsed + seconds.max(0.0)).min(self.duration);
        let t = self.elapsed / self.duration;
        for n in 0..128 {
            self.current[n] = self.source[n] + t * (self.target[n] - self.source[n]);
        }
        if t >= 1.0 {
            self.current = self.target;
            self.duration = 0.0;
        }
    }
    /// Current frequency in Hz of each MIDI note.
    pub fn frequencies(&self) -> [f64; 128] {
        self.current.map(f64::exp2)
    }
    /// Current log2 frequency in Hz of each MIDI note.
    pub fn log_table(&self) -> [f64; 128] {
        self.current
    }
    /// Morph progress from 0 to 1; 1 when no morph is in progress.
    pub fn progress(&self) -> f64 {
        if self.duration == 0.0 {
            1.0
        } else {
            self.elapsed / self.duration
        }
    }
}
