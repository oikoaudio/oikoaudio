//! A private editor draft; nothing reaches project state until validated Apply.
use crate::tuning::{Prepared, Preset, ValidatedScale};
use std::cell::RefCell;
#[derive(Clone)]
pub struct Draft {
    original: Preset,
    original_scale: ValidatedScale,
    prepared: RefCell<Option<(DraftState, Result<ValidatedScale, String>)>>,
    initial: Prepared,
    pub name: String,
    pub degrees: Vec<f64>,
    pub root: i32,
    pub reference: i32,
    reset_mapping: bool,
}
#[derive(Clone, PartialEq)]
pub struct DraftState {
    name: String,
    degrees: Vec<f64>,
    root: i32,
    reference: i32,
    reset_mapping: bool,
}
impl Draft {
    pub fn new(preset: Preset) -> Result<Self, String> {
        Ok(Self::from_validated(ValidatedScale::new(preset)?))
    }
    pub fn from_validated(scale: ValidatedScale) -> Self {
        let (preset, initial) = scale.parts().clone();
        Self {
            original_scale: scale,
            prepared: RefCell::new(None),
            name: preset.display_name.clone(),
            degrees: initial.degrees_cents.clone(),
            root: initial.root_note,
            reference: initial.reference_note,
            original: preset,
            initial,
            reset_mapping: false,
        }
    }
    pub fn reset_mapping(&mut self) {
        self.reset_mapping = true;
        self.root = 60;
        self.reference = 69;
    }
    pub fn has_custom_mapping(&self) -> bool {
        !self.reset_mapping && self.original.kbm_text.is_some()
    }
    pub fn state(&self) -> DraftState {
        DraftState {
            name: self.name.clone(),
            degrees: self.degrees.clone(),
            root: self.root,
            reference: self.reference,
            reset_mapping: self.reset_mapping,
        }
    }
    pub fn restore_state(&mut self, state: DraftState) {
        self.name = state.name;
        self.degrees = state.degrees;
        self.root = state.root;
        self.reference = state.reference;
        self.reset_mapping = state.reset_mapping;
    }
    pub fn equal_divisions(&mut self, count: usize, period: f64) -> Result<(), String> {
        if !(1..=4096).contains(&count) || !period.is_finite() || period <= 0. || period > 96000. {
            return Err("Use 1–4096 notes and a positive period up to 96000 cents.".into());
        }
        self.degrees = (1..=count)
            .map(|n| period * n as f64 / count as f64)
            .collect();
        self.reset_mapping();
        Ok(())
    }
    /// Cents of note `index` if the period were divided equally among the draft's notes.
    pub fn offset_baseline(&self, index: usize) -> f64 {
        self.degrees.last().copied().unwrap_or(0.) * (index + 1) as f64 / self.degrees.len() as f64
    }
    pub fn add_note(&mut self) -> Result<(), String> {
        self.insert_note(self.degrees.len() - 1)
    }
    /// Insert before an internal interval (or before the period), halfway from
    /// its predecessor. Root is implicit at zero; existing intervals stay put.
    pub fn insert_note(&mut self, index: usize) -> Result<(), String> {
        if self.degrees.len() >= 4096 {
            return Err("Use at most 4096 notes.".into());
        }
        if index >= self.degrees.len() {
            return Err("Invalid note position.".into());
        }
        let previous = if index == 0 {
            0.
        } else {
            self.degrees[index - 1]
        };
        self.degrees
            .insert(index, (previous + self.degrees[index]) / 2.);
        Ok(())
    }
    pub fn remove_note(&mut self, index: usize) -> Result<(), String> {
        if index >= self.degrees.len() - 1 {
            return Err("The root and period cannot be removed.".into());
        }
        self.degrees.remove(index);
        Ok(())
    }
    /// Keep `count` intervals starting at index `start`, measured from the note before
    /// `start`; the last kept interval becomes the period. Resets the keyboard mapping.
    pub fn extract_cycle(&mut self, start: usize, count: usize) -> Result<(), String> {
        let end = start.checked_add(count).ok_or("Invalid cycle range")?;
        if count == 0 || end > self.degrees.len() {
            return Err("Cycle must fit within the scale.".into());
        }
        let base = if start == 0 {
            0.
        } else {
            self.degrees[start - 1]
        };
        let cycle: Vec<_> = self.degrees[start..end].iter().map(|c| c - base).collect();
        if !cycle[count - 1].is_finite() || cycle[count - 1] <= 0. {
            return Err("The selected cycle must have a positive period.".into());
        }
        self.degrees = cycle;
        self.reset_mapping();
        Ok(())
    }
    pub fn preset(&self) -> Result<Preset, String> {
        Ok(self.validated()?.preset().clone())
    }
    /// Validate the current draft. Calls without an intervening edit return the same
    /// result, including the same error.
    pub fn validated(&self) -> Result<ValidatedScale, String> {
        let state = self.state();
        let mut cache = self.prepared.borrow_mut();
        if let Some((before, result)) = cache.as_ref()
            && before == &state
        {
            return result.clone();
        }
        let result = self.build_preset().and_then(|preset| {
            if preset == self.original {
                Ok(self.original_scale.clone())
            } else {
                ValidatedScale::new(preset)
            }
        });
        *cache = Some((state, result.clone()));
        result
    }
    fn build_preset(&self) -> Result<Preset, String> {
        if self.name.trim().is_empty() || self.name.contains(['\n', '\r', '\0']) {
            return Err("Give the scale a name on one line.".into());
        }
        if self.degrees.is_empty()
            || self.degrees.len() > 4096
            || self.degrees.iter().any(|v| !v.is_finite())
        {
            return Err("Use 1–4096 finite intervals.".into());
        }
        if !(0..=127).contains(&self.root) || !(-256..=255).contains(&self.reference) {
            return Err("Root must be 0–127; reference must be −256–255.".into());
        }
        let unchanged = self.degrees == self.initial.degrees_cents
            && self.root == self.initial.root_note
            && self.reference == self.initial.reference_note
            && !self.reset_mapping;
        let mut p = self.original.clone();
        p.display_name = self.name.trim().into();
        if unchanged && p.display_name == self.original.display_name {
            return Ok(p);
        }
        p.source_id = None;
        p.description = format!("Edited from {}.", self.original.display_name);
        if !unchanged {
            p.scl_text = format!(
                "! Edited in Oiko Inton\n{}\n{}\n{}",
                p.display_name,
                self.degrees.len(),
                self.degrees
                    .iter()
                    .map(|c| format!("{c:.12}\n"))
                    .collect::<String>()
            );
            let mut kbm = if self.reset_mapping {
                None
            } else {
                self.original.kbm_text.clone()
            };
            if let Some(text) = &mut kbm {
                let mut index = 0;
                *text = text
                    .lines()
                    .map(|line| {
                        if line.trim_start().starts_with('!') || line.trim().is_empty() {
                            return format!("{line}\n");
                        }
                        let out = match index {
                            3 => format!("{}\n", self.root),
                            4 => format!("{}\n", self.reference),
                            _ => format!("{line}\n"),
                        };
                        index += 1;
                        out
                    })
                    .collect();
            } else if self.root != 60 || self.reference != 69 {
                kbm = Some(format!(
                    "0\n0\n127\n{}\n{}\n440.0\n0\n",
                    self.root, self.reference
                ));
            }
            p.kbm_text = kbm;
        }
        Ok(p)
    }
}
