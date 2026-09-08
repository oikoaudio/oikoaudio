//! Local undo/redo snapshots and restoration.
use super::parameter_history::{ParameterChange, restore_parameters};
use crate::parameters::SpectralParams;
use crate::state::CurveState;
use spectral_dsp::MANUAL_MASK_POINTS;

const CURVE_HISTORY_LIMIT: usize = 24;
#[derive(Clone, PartialEq)]
pub(super) struct CurveSnapshot {
    pub(super) curve: Box<[f32]>,
    pub(super) depth_percent: f32,
    pub(super) tilt_db_per_octave: f32,
    pub(super) shift_semitones: f32,
}

impl CurveSnapshot {
    pub(super) fn capture(curve: &[f32; MANUAL_MASK_POINTS], params: &SpectralParams) -> Self {
        Self {
            curve: curve.to_vec().into_boxed_slice(),
            depth_percent: params.curve_depth_percent.value(),
            tilt_db_per_octave: params.curve_tilt_db_per_octave.value(),
            shift_semitones: params.curve_shift_semitones.value(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(super) enum HistoryEntry {
    Curve(CurveSnapshot),
    Notes(Box<crate::state::PinnedNotesSnapshot>),
    Parameters(Vec<ParameterChange>),
}

impl From<CurveSnapshot> for HistoryEntry {
    fn from(snapshot: CurveSnapshot) -> Self {
        Self::Curve(snapshot)
    }
}

impl HistoryEntry {
    fn inverse(&self, curve: CurveSnapshot, params: &SpectralParams) -> Self {
        match self {
            Self::Curve(_) => Self::Curve(curve),
            Self::Notes(_) => Self::Notes(Box::new(params.pinned_notes.snapshot())),
            Self::Parameters(changes) => {
                // Reverse requested values even before the host echoes the edit.
                let mut changes = changes.clone();
                for change in &mut changes {
                    change.reverse();
                }
                Self::Parameters(changes)
            }
        }
    }
}

#[derive(Default)]
pub(super) struct CurveHistory {
    pub(super) undo: Vec<HistoryEntry>,
    pub(super) redo: Vec<HistoryEntry>,
}

impl CurveHistory {
    pub(super) fn is_active(&self) -> bool {
        self.can_undo() || self.can_redo()
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(super) fn record_before(&mut self, snapshot: impl Into<HistoryEntry>) {
        let snapshot = snapshot.into();
        if self.undo.last() == Some(&snapshot) {
            self.redo.clear();
            return;
        }
        if self.undo.len() == CURVE_HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(snapshot);
        self.redo.clear();
    }

    pub(super) fn undo(
        &mut self,
        current: CurveSnapshot,
        params: &SpectralParams,
    ) -> Option<HistoryEntry> {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(previous.inverse(current, params));
            Some(previous)
        } else {
            None
        }
    }

    pub(super) fn redo(
        &mut self,
        current: CurveSnapshot,
        params: &SpectralParams,
    ) -> Option<HistoryEntry> {
        if let Some(next) = self.redo.pop() {
            if self.undo.len() == CURVE_HISTORY_LIMIT {
                self.undo.remove(0);
            }
            self.undo.push(next.inverse(current, params));
            Some(next)
        } else {
            None
        }
    }
}

pub(super) fn restore_curve(curve: &CurveState, snapshot: &[f32]) {
    for (index, db) in snapshot.iter().copied().enumerate() {
        curve.set_transformed(index, db);
    }
}

pub(super) fn restore_curve_snapshot(
    snapshot: HistoryEntry,
    curve: &CurveState,
    params: &SpectralParams,
    setter: &nice_plug::context::gui::ParamSetter<'_>,
) {
    let snapshot = match snapshot {
        HistoryEntry::Parameters(changes) => {
            restore_parameters(&changes, params, setter);
            return;
        }
        HistoryEntry::Notes(notes) => {
            params.pinned_notes.restore(&notes);
            return;
        }
        HistoryEntry::Curve(snapshot) => snapshot,
    };
    restore_curve(curve, &snapshot.curve);
    for (param, value) in [
        (&params.curve_depth_percent, snapshot.depth_percent),
        (
            &params.curve_tilt_db_per_octave,
            snapshot.tilt_db_per_octave,
        ),
        (&params.curve_shift_semitones, snapshot.shift_semitones),
    ] {
        if (param.value() - value).abs() > f32::EPSILON {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, value);
            setter.end_set_parameter(param);
        }
    }
}
