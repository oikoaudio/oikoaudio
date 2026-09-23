//! Records only explicit editor gestures, never audio-thread/host parameter updates.
use crate::parameters::SpectralParams;
use nice_plug::context::gui::ParamSetter;
use nice_plug::params::internals::ParamPtr;
use nice_plug::params::{Param, Params};
use oiko_plugin::gestures::ParameterWriter;
use std::cell::RefCell;

#[derive(Clone, PartialEq)]
pub(super) struct ParameterChange {
    id: String,
    before: f32,
    after: f32,
}

impl ParameterChange {
    pub(super) fn reverse(&mut self) {
        std::mem::swap(&mut self.before, &mut self.after);
    }
}

struct PendingChange {
    change: ParameterChange,
    active: bool,
    notified: bool,
}

#[derive(Default)]
pub(super) struct ParameterGestures {
    pending: RefCell<Vec<PendingChange>>,
    host: oiko_plugin::gestures::ParameterGestures,
}

impl ParameterGestures {
    pub(super) fn is_active(&self) -> bool {
        self.pending.borrow().iter().any(|change| change.active)
    }

    /// Called after drawing all controls. A compound action such as FREE/SYNC
    /// includes its conversion and mode switch in the same history entry.
    pub(super) fn take_completed(&self) -> Vec<ParameterChange> {
        if self.is_active() {
            return Vec::new();
        }
        self.pending
            .take()
            .into_iter()
            .map(|pending| pending.change)
            .filter(|change| change.before != change.after)
            .collect()
    }

    pub(super) fn finish(&self, params: &SpectralParams, setter: &ParamSetter<'_>) {
        self.host.finish(params, setter);
        for pending in self.pending.borrow_mut().iter_mut() {
            pending.active = false;
            pending.notified = false;
        }
    }
}

pub(super) struct TrackedParamSetter<'a> {
    setter: &'a ParamSetter<'a>,
    gestures: &'a ParameterGestures,
    map: Vec<(String, ParamPtr, String)>,
}

impl<'a> TrackedParamSetter<'a> {
    pub(super) fn new(
        setter: &'a ParamSetter<'a>,
        params: &'a SpectralParams,
        gestures: &'a ParameterGestures,
    ) -> Self {
        Self {
            setter,
            gestures,
            map: params.param_map(),
        }
    }

    fn id<P: Param>(&self, param: &P) -> &str {
        &self
            .map
            .iter()
            .find(|(_, ptr, _)| *ptr == param.as_ptr())
            .expect("Editor control belongs to SpectralParams")
            .0
    }
}

impl ParameterWriter for TrackedParamSetter<'_> {
    fn begin_set_parameter<P: Param>(&self, param: &P) {
        let id = self.id(param);
        let mut pending = self.gestures.pending.borrow_mut();
        if let Some(change) = pending.iter_mut().find(|pending| pending.change.id == id) {
            change.active = true;
        } else {
            let before = param.unmodulated_normalized_value();
            pending.push(PendingChange {
                change: ParameterChange {
                    id: id.to_owned(),
                    before,
                    after: before,
                },
                active: true,
                notified: false,
            });
        }
    }

    fn set_parameter<P: Param>(&self, param: &P, value: P::Plain) {
        let id = self.id(param);
        let next = param.preview_normalized(value);
        let mut pending = self.gestures.pending.borrow_mut();
        let change = pending
            .iter_mut()
            .find(|pending| pending.change.id == id)
            .expect("Parameter changes must have a gesture");
        // Use the last requested value: CLAP/VST3 may not have echoed it yet.
        if change.change.after == next {
            return;
        }
        if !change.notified {
            self.gestures
                .host
                .setter(self.setter)
                .begin_set_parameter(param);
            change.notified = true;
        }
        self.gestures
            .host
            .setter(self.setter)
            .set_parameter_normalized(param, next);
        change.change.after = next;
    }

    fn end_set_parameter<P: Param>(&self, param: &P) {
        let id = self.id(param);
        let mut pending = self.gestures.pending.borrow_mut();
        if let Some(change) = pending.iter_mut().find(|pending| pending.change.id == id) {
            if change.notified {
                self.gestures
                    .host
                    .setter(self.setter)
                    .end_set_parameter(param);
            }
            change.active = false;
            change.notified = false;
        }
    }
}

pub(super) fn restore_parameters(
    changes: &[ParameterChange],
    params: &SpectralParams,
    setter: &ParamSetter<'_>,
) {
    let map = params.param_map();
    for change in changes {
        if let Some((_, ptr, _)) = map.iter().find(|(id, _, _)| *id == change.id) {
            // Do not skip based on its current value: a previous queued edit
            // may still be in flight. All pointers belong to borrowed params.
            unsafe {
                setter.raw_context.raw_begin_set_parameter(*ptr);
                setter
                    .raw_context
                    .raw_set_parameter_normalized(*ptr, change.before);
                setter.raw_context.raw_end_set_parameter(*ptr);
            }
        }
    }
}

#[cfg(test)]
mod tests;
