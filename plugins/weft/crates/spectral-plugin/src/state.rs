//! State preflight runs before the wrapper mutates any live parameters/fields.
//! Parsing and parameter metadata allocation belong to host state loading, not
//! the process/reset paths. Missing historical fields keep existing defaults.
use crate::curve::CURVE_TRANSFORM_STORAGE_LIMIT_DB;
use crate::parameters::SpectralParams;
use nice_plug::params::persist::PersistentField;
use nice_plug::prelude::{Params, PluginState};
use spectral_dsp::{MANUAL_CURVE_MUTE_DB, MANUAL_MASK_POINTS, MIDI_NOTES};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

pub(crate) type UiScaleState = oiko_plugin::UiScaleState<125>;
use nice_plug::params::{internals::ParamPtr, persist::deserialize_field};
use nice_plug::plugin::ParamValue;

pub(super) fn validate(state: &PluginState) -> Result<(), String> {
    let defaults = SpectralParams::default();
    for (id, parameter, _) in defaults.param_map() {
        let Some(value) = state.params.get(&id) else {
            continue;
        };
        // All pointers are owned by `defaults`, which outlives this entire loop.
        let valid = unsafe {
            match (parameter, value) {
                (ParamPtr::FloatParam(p), ParamValue::F32(v)) => {
                    v.is_finite() && ((*p).preview_plain(0.0)..=(*p).preview_plain(1.0)).contains(v)
                }
                (ParamPtr::IntParam(p), ParamValue::I32(v)) => {
                    ((*p).preview_plain(0.0)..=(*p).preview_plain(1.0)).contains(v)
                }
                (ParamPtr::BoolParam(_), ParamValue::Bool(_)) => true,
                (ParamPtr::EnumParam(p), ParamValue::I32(v)) => {
                    *v >= 0 && (*v as usize) < (*p).len()
                }
                (ParamPtr::EnumParam(p), ParamValue::String(v)) => (*p).set_from_id(v),
                _ => false,
            }
        };
        if !valid {
            return Err(format!("Invalid Weft parameter: {id}"));
        }
    }
    for id in ["manual-curve-v1", "pinned-notes-v1"] {
        if let Some(json) = state.fields.get(id) {
            let values: Vec<f32> =
                deserialize_field(json).map_err(|_| format!("Invalid Weft field: {id}"))?;
            if values.iter().any(|v| !v.is_finite()) {
                return Err(format!("Nonfinite Weft field: {id}"));
            }
        }
    }
    for id in ["ui-scale-v1", "spectrum-range-v1"] {
        if let Some(json) = state.fields.get(id) {
            let value: f32 =
                deserialize_field(json).map_err(|_| format!("Invalid Weft field: {id}"))?;
            if !value.is_finite() {
                return Err(format!("Nonfinite Weft field: {id}"));
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) struct CurveState {
    points: Arc<[AtomicU32; MANUAL_MASK_POINTS]>,
}

impl Default for CurveState {
    fn default() -> Self {
        Self {
            points: Arc::new([const { AtomicU32::new(0.0_f32.to_bits()) }; MANUAL_MASK_POINTS]),
        }
    }
}

impl CurveState {
    #[cfg(test)]
    pub(crate) fn get(&self, index: usize) -> f32 {
        f32::from_bits(self.points[index].load(Ordering::Relaxed))
    }

    pub(crate) fn set(&self, index: usize, db: f32) {
        self.points[index].store(
            db.clamp(MANUAL_CURVE_MUTE_DB, 0.0).to_bits(),
            Ordering::Release,
        );
    }

    pub(crate) fn set_transformed(&self, index: usize, db: f32) {
        self.points[index].store(
            db.clamp(
                -CURVE_TRANSFORM_STORAGE_LIMIT_DB,
                CURVE_TRANSFORM_STORAGE_LIMIT_DB,
            )
            .to_bits(),
            Ordering::Release,
        );
    }

    pub(crate) fn reset(&self) {
        for point in self.points.iter() {
            point.store(0.0_f32.to_bits(), Ordering::Release);
        }
    }

    pub(crate) fn copy_to(&self, output: &mut [f32; MANUAL_MASK_POINTS]) {
        for (output, point) in output.iter_mut().zip(self.points.iter()) {
            *output = f32::from_bits(point.load(Ordering::Acquire));
        }
    }
}

impl<'a> PersistentField<'a, Vec<f32>> for CurveState {
    fn set(&self, new_value: Vec<f32>) {
        if new_value.is_empty() {
            self.reset();
            return;
        }
        for (index, point) in self.points.iter().enumerate() {
            let db = if new_value.len() == MANUAL_MASK_POINTS {
                new_value[index]
            } else {
                // Migrate the original 128-point logarithmic curve (and any
                // future differently sized curve) into the linear FFT-bin
                // master mask. State restoration has no sample-rate context,
                // so use the POC's 48 kHz reference range here.
                let frequency = (index as f32 / (MANUAL_MASK_POINTS - 1) as f32 * 24_000.0)
                    .clamp(20.0, 24_000.0);
                let normalized = (frequency / 20.0).ln() / (24_000.0_f32 / 20.0).ln();
                let position = normalized * (new_value.len() - 1) as f32;
                let left = position.floor() as usize;
                let right = (left + 1).min(new_value.len() - 1);
                let fraction = position - left as f32;
                new_value[left] + (new_value[right] - new_value[left]) * fraction
            }
            .clamp(
                -CURVE_TRANSFORM_STORAGE_LIMIT_DB,
                CURVE_TRANSFORM_STORAGE_LIMIT_DB,
            );
            point.store(db.to_bits(), Ordering::Release);
        }
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Vec<f32>) -> R,
    {
        let snapshot = self
            .points
            .iter()
            .map(|point| f32::from_bits(point.load(Ordering::Acquire)))
            .collect();
        f(&snapshot)
    }
}

#[derive(Clone)]
pub(crate) struct PinnedNotesState {
    notes: Arc<[AtomicBool; MIDI_NOTES]>,
    held_notes: Arc<[AtomicBool; MIDI_NOTES]>,
    capture_incoming: Arc<AtomicBool>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct PinnedNotesSnapshot {
    notes: [bool; MIDI_NOTES],
    held: [bool; MIDI_NOTES],
    holding: bool,
}

impl Default for PinnedNotesState {
    fn default() -> Self {
        Self {
            notes: Arc::new([const { AtomicBool::new(false) }; MIDI_NOTES]),
            held_notes: Arc::new([const { AtomicBool::new(false) }; MIDI_NOTES]),
            capture_incoming: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl PinnedNotesState {
    pub(crate) fn snapshot(&self) -> PinnedNotesSnapshot {
        PinnedNotesSnapshot {
            notes: std::array::from_fn(|i| self.notes[i].load(Ordering::Acquire)),
            held: std::array::from_fn(|i| self.held_notes[i].load(Ordering::Acquire)),
            holding: self.capture_incoming(),
        }
    }

    pub(crate) fn restore(&self, snapshot: &PinnedNotesSnapshot) {
        for i in 0..MIDI_NOTES {
            self.notes[i].store(snapshot.notes[i], Ordering::Release);
            self.held_notes[i].store(snapshot.held[i], Ordering::Release);
        }
        self.capture_incoming
            .store(snapshot.holding, Ordering::Release);
    }
    pub(crate) fn get(&self, note: usize) -> bool {
        self.notes[note].load(Ordering::Acquire) || self.held_notes[note].load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn set(&self, note: usize, pinned: bool) {
        self.notes[note].store(pinned, Ordering::Release);
    }

    pub(crate) fn set_from_ui(&self, note: usize, pinned: bool) {
        self.notes[note].store(pinned, Ordering::Release);
        if !pinned {
            self.held_notes[note].store(false, Ordering::Release);
        }
    }

    pub(crate) fn clear(&self) {
        for note in self.notes.iter() {
            note.store(false, Ordering::Release);
        }
        self.clear_held();
    }

    pub(crate) fn capture_incoming(&self) -> bool {
        self.capture_incoming.load(Ordering::Acquire)
    }

    pub(crate) fn toggle_capture_incoming(&self) {
        let was_enabled = self.capture_incoming.fetch_xor(true, Ordering::AcqRel);
        if was_enabled {
            self.clear_held();
        }
    }

    pub(crate) fn capture(&self, note: usize) {
        if self.capture_incoming() {
            self.held_notes[note].store(true, Ordering::Release);
        }
    }

    fn clear_held(&self) {
        for note in self.held_notes.iter() {
            note.store(false, Ordering::Release);
        }
    }
}

impl<'a> PersistentField<'a, Vec<f32>> for PinnedNotesState {
    fn set(&self, new_value: Vec<f32>) {
        for (index, note) in self.notes.iter().enumerate() {
            note.store(
                new_value.get(index).copied().unwrap_or(0.0) >= 0.5,
                Ordering::Release,
            );
        }
        self.held_notes
            .iter()
            .for_each(|note| note.store(false, Ordering::Release));
        self.capture_incoming.store(false, Ordering::Release);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Vec<f32>) -> R,
    {
        let snapshot: Vec<f32> = self
            .notes
            .iter()
            .map(|note| f32::from(note.load(Ordering::Acquire)))
            .collect();
        f(&snapshot)
    }
}

#[derive(Clone)]
pub(crate) struct SpectrumRangeState {
    value: Arc<AtomicU32>,
}

impl Default for SpectrumRangeState {
    fn default() -> Self {
        Self {
            value: Arc::new(AtomicU32::new(60)),
        }
    }
}

impl SpectrumRangeState {
    pub(crate) fn get(&self) -> f32 {
        self.value.load(Ordering::Acquire) as f32
    }

    pub(crate) fn set(&self, range_db: f32) {
        let closest = [30_u32, 60, 90, 144]
            .into_iter()
            .min_by(|left, right| {
                (*left as f32 - range_db)
                    .abs()
                    .total_cmp(&(*right as f32 - range_db).abs())
            })
            .unwrap_or(60);
        self.value.store(closest, Ordering::Release);
    }
}

impl<'a> PersistentField<'a, f32> for SpectrumRangeState {
    fn set(&self, new_value: f32) {
        self.set(new_value);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&f32) -> R,
    {
        f(&self.get())
    }
}

#[cfg(test)]
mod tests;
