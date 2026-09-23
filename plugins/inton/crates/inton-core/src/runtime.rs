use crate::engine::Parameter;
use crate::{
    engine::{Engine, Parameters},
    mts::{Master, MasterStatus, Native},
    state::{Project, ValidatedProject},
    tuning::{Preset, ValidatedScale, default_scale},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::SeqCst},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
/// Lock-free exchange of the automatable parameters between host callbacks and the
/// control thread. `set_host_value` never blocks. A control-thread write replaces every
/// value and discards pending host values; reads return `None` while a write is in progress.
pub struct ParameterMailbox {
    generation: AtomicU64,
    values: [AtomicU64; Parameter::COUNT],
    host_updates: [AtomicU64; Parameter::COUNT],
}
impl Default for ParameterMailbox {
    fn default() -> Self {
        Self::new()
    }
}
impl ParameterMailbox {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            host_updates: std::array::from_fn(|_| AtomicU64::new(f64::NAN.to_bits())),
            values: Parameters::default()
                .values()
                .map(|v| AtomicU64::new(v.to_bits())),
        }
    }
    pub fn try_write(&self, p: Parameters) -> bool {
        self.try_write_versioned(p, self.generation.load(SeqCst))
    }
    pub fn try_write_versioned(&self, p: Parameters, before: u64) -> bool {
        if !before.is_multiple_of(2)
            || self
                .generation
                .compare_exchange(before, before.wrapping_add(1), SeqCst, SeqCst)
                .is_err()
        {
            return false;
        }
        // A complete state replacement supersedes earlier host updates.
        for update in &self.host_updates {
            update.store(f64::NAN.to_bits(), SeqCst);
        }
        for (a, v) in self.values.iter().zip(p.values()) {
            a.store(v.to_bits(), SeqCst)
        }
        self.generation.store(before.wrapping_add(2), SeqCst);
        true
    }
    /// Replace all values and discard pending host values. Spins until any concurrent
    /// control-thread write finishes, so never call this from the audio thread.
    pub fn write(&self, p: Parameters) {
        while !self.try_write(p) {
            std::thread::yield_now();
        }
    }
    pub fn read(&self) -> Option<Parameters> {
        self.read_versioned().map(|(p, _)| p)
    }
    pub fn read_versioned(&self) -> Option<(Parameters, u64)> {
        let before = self.generation.load(SeqCst);
        if !before.is_multiple_of(2) {
            return None;
        }
        let v = Parameter::ALL.map(|id| self.value(id));
        if self.generation.load(SeqCst) != before {
            return None;
        }
        Some((Parameters::from_values(v), before))
    }
    /// Apply `update` to the current values after folding in pending host values.
    /// Spins while another control-thread write is in progress; host callbacks never wait.
    fn modify(&self, update: impl FnOnce(Parameters) -> Parameters) -> Parameters {
        let generation = loop {
            let before = self.generation.load(SeqCst);
            if before.is_multiple_of(2)
                && self
                    .generation
                    .compare_exchange(before, before.wrapping_add(1), SeqCst, SeqCst)
                    .is_ok()
            {
                break before;
            }
            std::thread::yield_now();
        };
        let p = Parameters::from_values(std::array::from_fn(|i| {
            let update = f64::from_bits(self.host_updates[i].swap(f64::NAN.to_bits(), SeqCst));
            if update.is_finite() {
                update
            } else {
                f64::from_bits(self.values[i].load(SeqCst))
            }
        }));
        let updated = update(p);
        for (a, v) in self.values.iter().zip(updated.values()) {
            a.store(v.to_bits(), SeqCst);
        }
        self.generation.store(generation.wrapping_add(2), SeqCst);
        updated
    }
    /// Record the latest host value; realtime-safe (no locks, retries, allocation or wakeup).
    /// Non-finite values are ignored. The value takes precedence over the stored one until
    /// the next control-thread write consumes or discards it.
    pub fn set_host_value(&self, id: Parameter, value: f64) {
        if value.is_finite() {
            self.host_updates[id.index()].store(value.to_bits(), SeqCst);
        }
    }

    pub fn value(&self, id: Parameter) -> f64 {
        let i = id.index();
        let update = f64::from_bits(self.host_updates[i].load(SeqCst));
        if update.is_finite() {
            update
        } else {
            f64::from_bits(self.values[i].load(SeqCst))
        }
    }
}
/// Independent latest-value edits from control/UI writers to one editor consumer.
/// Publish the value before its mask bit. A drain claims bits once; edits arriving
/// during consumption remain pending, and may coalesce with an already claimed edit.
/// This is not a coherent multi-parameter transaction. No allocation or waiting.
struct PendingParameterEdits {
    values: [AtomicU64; Parameter::COUNT],
    mask: AtomicU32,
}
impl Default for PendingParameterEdits {
    fn default() -> Self {
        Self {
            values: std::array::from_fn(|_| AtomicU64::new(0)),
            mask: AtomicU32::new(0),
        }
    }
}
impl PendingParameterEdits {
    fn set(&self, id: Parameter, value: f64) {
        self.values[id.index()].store(value.to_bits(), SeqCst);
        self.mask.fetch_or(1 << id.index(), SeqCst);
    }
    fn take(&self) -> impl Iterator<Item = (Parameter, f64)> + '_ {
        let mask = self.mask.swap(0, SeqCst);
        Parameter::ALL
            .into_iter()
            .filter(move |id| mask & (1 << id.index()) != 0)
            .map(move |id| (id, f64::from_bits(self.values[id.index()].load(SeqCst))))
    }
    fn clear(&self) {
        self.mask.store(0, SeqCst);
    }
}

pub struct Session {
    pub project: Project,
    pub engine: Engine,
}
impl Session {
    fn snapshot(&mut self, parameters: Parameters) -> Project {
        self.engine.update(parameters, &self.project, false);
        let mut saved = self.project.clone();
        saved.parameters = parameters;
        saved.held_log = self.engine.empty.then(|| self.engine.log_table().to_vec());
        saved.held_name = self.engine.empty.then(|| self.engine.name.clone());
        saved
    }
}
#[derive(Default)]
struct EditHistory {
    undo: Vec<(String, ValidatedProject, usize)>,
    redo: Vec<(String, ValidatedProject, usize)>,
}

/// Project, tuning engine and MTS state shared by the plugin, its editor and the MTS thread.
pub struct Shared {
    history: Mutex<EditHistory>,
    pub audition_enabled: AtomicBool,
    /// Set when a blocked MTS master registration can be reset with `recovery_requested`.
    pub recovery_available: AtomicBool,
    /// Set to make the MTS thread reinitialize libMTS on its next update; cleared once handled.
    pub recovery_requested: AtomicBool,
    audition: Mutex<Option<ValidatedScale>>,
    pub parameters: ParameterMailbox,
    pub project: Mutex<Session>,
    /// Latest MTS master status and connected client count.
    pub status: Mutex<(MasterStatus, usize)>,
    pub error: Mutex<Option<String>>,
    pending: PendingParameterEdits,
    /// Set to stop the thread started by `start`.
    pub stop: AtomicBool,
}
impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
impl Shared {
    pub fn new() -> Self {
        let project = Project::default();
        let engine = Engine::new(&project).expect("factory validated");
        Self {
            history: Mutex::new(EditHistory::default()),
            audition_enabled: AtomicBool::new(false),
            recovery_available: AtomicBool::new(false),
            recovery_requested: AtomicBool::new(false),
            audition: Mutex::new(None),
            parameters: ParameterMailbox::new(),
            project: Mutex::new(Session { project, engine }),
            status: Mutex::new((MasterStatus::Unavailable, 0)),
            error: Mutex::new(None),
            pending: PendingParameterEdits::default(),
            stop: AtomicBool::new(false),
        }
    }
    pub fn audition_scale(&self, preset: Preset) -> Result<(), String> {
        self.audition_prepared(ValidatedScale::new(preset)?);
        Ok(())
    }
    pub fn audition_prepared(&self, scale: ValidatedScale) {
        if self.audition_enabled.load(SeqCst) {
            *self.audition.lock().unwrap() = Some(scale);
        }
    }
    pub fn scale(&self, index: usize) -> Option<ValidatedScale> {
        self.project.lock().unwrap().engine.scale(index)
    }
    pub fn stop_audition(&self) {
        self.audition_enabled.store(false, SeqCst);
        *self.audition.lock().unwrap() = None;
    }
    /// Advance the morph by `dt` seconds and return what to publish: (frequencies in Hz per
    /// MIDI note, scale name, period ratio, enabled). While audition is active this returns
    /// the auditioned scale without changing project slots or automation.
    pub fn publication(&self, dt: f64) -> ([f64; 128], String, f64, bool) {
        let mut s = self.project.lock().unwrap();
        let Session { project, engine } = &mut *s;
        engine.advance(dt);
        if let Some(p) = self.parameters.read() {
            engine.update(p, project, false);
        }
        if self.audition_enabled.load(SeqCst)
            && let Some(scale) = self.audition.lock().unwrap().as_ref()
        {
            let (preset, tuning) = scale.parts();
            let multiplier = engine.parameters.reference / 440.0
                * 2.0_f64.powf(engine.parameters.transpose as f64 / 12.0);
            return (
                tuning.hz.map(|hz| hz * multiplier),
                preset.display_name.clone(),
                tuning.period,
                engine.parameters.enabled,
            );
        }
        (
            engine.frequencies(),
            engine.name.clone(),
            engine.period,
            engine.parameters.enabled,
        )
    }
    /// Queue a parameter change made by the editor or control code for the host.
    /// Unlike `set_parameter_value`, it does not change the value directly.
    pub fn edit_parameter(&self, id: Parameter, value: f64) {
        self.pending.set(id, value);
    }
    /// Drain edits queued with `edit_parameter` since the last call, one latest value per
    /// parameter. The caller forwards them to the host.
    pub fn take_parameter_edits(&self) -> impl Iterator<Item = (Parameter, f64)> + '_ {
        self.pending.take()
    }
    /// Apply a parameter value received from the host. Realtime-safe.
    pub fn set_parameter_value(&self, id: Parameter, value: f64) {
        self.parameters.set_host_value(id, value);
    }
    pub fn enable_scale_set(&self) {
        let mut session = self.project.lock().unwrap();
        session.project.scale_set = true;
        session.project.set_collapsed = false;
    }
    pub fn fold_scale_set(&self, collapsed: bool) {
        self.project.lock().unwrap().project.set_collapsed = collapsed;
    }
    pub fn assign(&self, index: usize, preset: Preset) -> Result<(), String> {
        self.assign_prepared(index, ValidatedScale::new(preset)?)
    }
    pub fn assign_prepared(&self, index: usize, scale: ValidatedScale) -> Result<(), String> {
        let before = self.prepared_snapshot();
        self.assign_inner(index, scale)?;
        self.record_edit("Replace scale", before);
        Ok(())
    }
    fn assign_inner(&self, index: usize, scale: ValidatedScale) -> Result<(), String> {
        let mut s = self.project.lock().map_err(|e| e.to_string())?;
        s.project.assign_prepared(index, &scale)?;
        let Session { project, engine } = &mut *s;
        engine.replace_scale(index, Some(scale));
        engine.update(
            self.parameters.read().unwrap_or(engine.parameters),
            project,
            index == engine.parameters.position,
        );
        Ok(())
    }
    /// Put the scale in the first slot not yet added to the scale set, as one undo step.
    /// Returns the slot index; fails when the scale is invalid or all slots are in use.
    pub fn append(&self, preset: Preset) -> Result<usize, String> {
        self.append_prepared(ValidatedScale::new(preset)?)
    }
    pub fn append_prepared(&self, scale: ValidatedScale) -> Result<usize, String> {
        let before = self.prepared_snapshot();
        let result = self.append_inner(scale)?;
        self.record_edit("Add scale", before);
        Ok(result)
    }
    fn append_inner(&self, scale: ValidatedScale) -> Result<usize, String> {
        let mut s = self.project.lock().map_err(|e| e.to_string())?;
        let index = (0..crate::state::SET_SIZE)
            .find(|&n| !s.project.has_slot(n))
            .ok_or("All 32 slots have been added. Select a slot to replace its contents.")?;
        s.project.assign_prepared(index, &scale)?;
        let Session { project, engine } = &mut *s;
        engine.replace_scale(index, Some(scale));
        let parameters = self.parameters.read().unwrap_or(engine.parameters);
        engine.update(parameters, project, index == parameters.position);
        Ok(index)
    }
    /// Reorder contents, preserving the current tuning table and morph trajectory.
    /// Future host automation continues to address the fixed numerical positions.
    pub fn move_slot(&self, from: usize, to: usize) -> Result<(), String> {
        let before = self.prepared_snapshot();
        self.move_slot_inner(from, to)?;
        self.record_edit("Reorder scales", before);
        Ok(())
    }
    fn move_slot_inner(&self, from: usize, to: usize) -> Result<(), String> {
        let mut s = self.project.lock().map_err(|e| e.to_string())?;
        if from >= crate::state::SET_SIZE
            || to >= crate::state::SET_SIZE
            || s.project.slots[from].is_none()
        {
            return Err("Invalid slot move".into());
        }
        if from == to {
            return Ok(());
        }
        let Session { project, engine } = &mut *s;
        let mut old_position = engine.parameters.position;
        let parameters = self.parameters.modify(|mut parameters| {
            engine.update(parameters, project, false);
            if from < to {
                project.slots[from..=to].rotate_left(1);
            } else {
                project.slots[to..=from].rotate_right(1);
            }
            for n in from.min(to)..=from.max(to) {
                if project.slots[n].is_some() {
                    project.allocated_slots |= 1 << n;
                }
            }
            engine.move_slot(from, to);
            old_position = parameters.position;
            parameters.position = crate::state::moved_index(old_position, from, to);
            project.parameters = parameters;
            parameters
        });
        if parameters.position != old_position {
            self.edit_parameter(Parameter::Position, parameters.position as f64);
        }
        Ok(())
    }
    pub fn clear(&self, index: usize) -> Result<(), String> {
        let before = self.prepared_snapshot();
        self.clear_inner(index)?;
        self.record_edit("Clear slot", before);
        Ok(())
    }
    fn clear_inner(&self, index: usize) -> Result<(), String> {
        let mut s = self.project.lock().map_err(|e| e.to_string())?;
        let removed = s.engine.scale(index);
        s.project.clear(index)?;
        let return_to_single = s.project.slots.iter().all(Option::is_none);
        if return_to_single {
            s.project.allocated_slots = 0;
            s.project.scale_set = false;
            s.project.set_collapsed = false;
            s.project
                .assign_prepared(0, &removed.clone().unwrap_or_else(default_scale))?;
        }
        let Session { project, engine } = &mut *s;
        engine.replace_scale(index, None);
        if return_to_single {
            engine.replace_scale(0, Some(removed.unwrap_or_else(default_scale)));
        }
        let mut changed_position = false;
        let parameters = self.parameters.modify(|mut parameters| {
            if return_to_single {
                parameters.position = 0;
                changed_position = true;
            }
            if parameters.position == index
                && let Some(first) = project.slots.iter().position(Option::is_some)
            {
                parameters.position = first;
                changed_position = true;
            }
            project.parameters = parameters;
            engine.update(
                parameters,
                project,
                parameters.position == index || changed_position,
            );
            parameters
        });
        if changed_position {
            self.edit_parameter(Parameter::Position, parameters.position as f64);
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Project {
        let mut session = self.project.lock().unwrap();
        let p = self.parameters.read().unwrap_or(session.engine.parameters);
        session.snapshot(p)
    }
    fn prepared_snapshot(&self) -> ValidatedProject {
        let mut session = self.project.lock().unwrap();
        let p = self.parameters.read().unwrap_or(session.engine.parameters);
        ValidatedProject {
            project: session.snapshot(p),
            scales: (0..crate::state::SET_SIZE)
                .map(|i| session.engine.scale(i))
                .collect(),
        }
    }
    /// Replace the whole project. Discards pending host values and parameter edits, stops
    /// audition and clears undo history. Not realtime-safe.
    pub fn restore(&self, project: Project) -> Result<(), String> {
        self.restore_prepared(ValidatedProject::new(project)?)
    }
    pub fn restore_prepared(&self, project: ValidatedProject) -> Result<(), String> {
        self.restore_prepared_inner(project)?;
        *self.history.lock().unwrap() = EditHistory::default();
        Ok(())
    }
    fn restore_prepared_inner(&self, prepared: ValidatedProject) -> Result<(), String> {
        let engine = Engine::from_validated(&prepared);
        let project = prepared.project;
        self.stop_audition();
        let mut session = self.project.lock().map_err(|e| e.to_string())?;
        self.parameters.write(project.parameters);
        self.pending.clear();
        *session = Session { project, engine };
        Ok(())
    }
    fn record_edit(&self, label: &str, before: ValidatedProject) {
        let after = self.snapshot();
        if before.project.slots == after.slots
            && before.project.allocated_slots == after.allocated_slots
            && before.project.scale_set == after.scale_set
        {
            return;
        }
        let mut history = self.history.lock().unwrap();
        history.redo.clear();
        if history.undo.len() == 32 {
            history.undo.remove(0);
        }
        history
            .undo
            .push((label.into(), before, after.parameters.position));
    }
    /// Labels of the next undo and redo steps, in that order.
    pub fn history_labels(&self) -> (Option<String>, Option<String>) {
        let history = self.history.lock().unwrap();
        (
            history.undo.last().map(|e| e.0.clone()),
            history.redo.last().map(|e| e.0.clone()),
        )
    }
    /// Undo the latest edit, or redo the latest undone edit when `redo` is true; does
    /// nothing when there is none. Current parameter values are kept, except that the
    /// selected slot is restored if it has not changed since the edit.
    pub fn undo_edit(&self, redo: bool) -> Result<(), String> {
        let current = self.prepared_snapshot();
        let mut history = self.history.lock().unwrap();
        let source = if redo {
            &mut history.redo
        } else {
            &mut history.undo
        };
        let Some((label, saved, expected_position)) = source.last().cloned() else {
            return Ok(());
        };
        let mut restored = saved;
        let position = if current.project.parameters.position == expected_position {
            restored.project.parameters.position
        } else {
            current.project.parameters.position
        };
        restored.project.parameters = current.project.parameters;
        restored.project.parameters.position = position;
        self.restore_prepared_inner(restored)?;
        source.pop();
        let destination = if redo {
            &mut history.undo
        } else {
            &mut history.redo
        };
        destination.push((label, current, position));
        self.edit_parameter(Parameter::Position, position as f64);
        Ok(())
    }
    /// Add scales to the first free slots as one undo step and return their indices.
    /// When any scale is invalid or there is not enough space, nothing is added.
    pub fn append_scales(&self, presets: Vec<Preset>) -> Result<Vec<usize>, String> {
        let before = self.prepared_snapshot();
        let mut after = before.clone();
        let mut indices = Vec::new();
        for preset in presets {
            let index = (0..crate::state::SET_SIZE)
                .find(|&n| !after.project.has_slot(n))
                .ok_or("Not enough free slots in the scale set")?;
            let scale = ValidatedScale::new(preset)?;
            after.project.assign_prepared(index, &scale)?;
            after.scales[index] = Some(scale);
            indices.push(index);
        }
        if !indices.is_empty() {
            self.restore_prepared_inner(after)?;
            self.record_edit("Add scales", before);
        }
        Ok(indices)
    }
    pub fn clear_scale_set(&self) -> Result<(), String> {
        let before = self.prepared_snapshot();
        // If the active tuning is in no slot, slot 0 stays empty and the held table keeps
        // it sounding.
        let scale = before.scales[before.project.parameters.position]
            .clone()
            .or_else(|| {
                before
                    .scales
                    .iter()
                    .flatten()
                    .find(|s| Some(&s.preset().display_name) == before.project.held_name.as_ref())
                    .cloned()
            });
        let mut cleared = before.clone();
        cleared.project.slots = vec![None; crate::state::SET_SIZE];
        cleared.project.slots[0] = scale.as_ref().map(|s| s.preset().clone());
        cleared.scales = vec![None; crate::state::SET_SIZE];
        cleared.scales[0] = scale;
        cleared.project.allocated_slots = 1;
        cleared.project.scale_set = false;
        cleared.project.set_collapsed = false;
        cleared.project.parameters.position = 0;
        self.restore_prepared_inner(cleared)?;
        self.edit_parameter(Parameter::Position, 0.);
        self.record_edit("Clear scale set", before);
        Ok(())
    }
    /// Start the MTS thread. It loads libMTS, then about every 5 ms advances the morph,
    /// publishes the tuning and updates `status`, until `stop` is set.
    pub fn start(self: &Arc<Self>) -> std::io::Result<JoinHandle<()>> {
        let shared = self.clone();
        thread::Builder::new()
            .name("inton-mts".into())
            .spawn(move || {
                let mut master = Master::new(Native::load());
                let mut last = Instant::now();
                while !shared.stop.load(SeqCst) {
                    let now = Instant::now();
                    let dt = now.duration_since(last).as_secs_f64();
                    last = now;
                    let (hz, name, period, enabled) = shared.publication(dt);
                    let status = if shared.recovery_requested.swap(false, SeqCst) {
                        master.recover_and_update(enabled, &hz, &name, period)
                    } else {
                        master.update(enabled, &hz, &name, period)
                    };
                    shared.recovery_available.store(
                        status == MasterStatus::BlockedByOtherMaster && master.recovery_available(),
                        SeqCst,
                    );
                    *shared.status.lock().unwrap() = (status, master.clients());
                    thread::sleep(Duration::from_millis(5));
                }
            })
    }
}

#[cfg(test)]
mod mailbox_tests;
