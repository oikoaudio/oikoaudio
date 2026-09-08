//! Host gesture lifetimes. Call from the GUI thread, including editor teardown.
use nice_plug::{
    context::gui::ParamSetter,
    params::{Param, Params, internals::ParamPtr},
};
use std::sync::Mutex;

/// Tracks notifications only. Product undo history belongs to the product.
#[derive(Default)]
pub struct ParameterGestures {
    active: Mutex<Vec<ParamPtr>>,
}
impl ParameterGestures {
    pub fn setter<'a>(&'a self, setter: &'a ParamSetter<'a>) -> GestureSetter<'a> {
        GestureSetter {
            setter,
            gestures: self,
        }
    }

    /// End remaining gestures on pointer release, lost focus, or editor close.
    /// Resolve pointers against the live parameter owner before notifying the host.
    pub fn finish(&self, params: &dyn Params, setter: &ParamSetter<'_>) {
        let active = std::mem::take(&mut *self.active.lock().unwrap());
        if active.is_empty() {
            return;
        }
        let map = params.param_map();
        for ptr in active {
            if map.iter().any(|(_, live, _)| *live == ptr) {
                // This pointer belongs to the live params object borrowed above.
                unsafe { setter.raw_context.raw_end_set_parameter(ptr) };
            }
        }
    }
}

/// A host parameter writer, optionally recording product-local history.
pub trait ParameterWriter {
    fn begin_set_parameter<P: Param>(&self, param: &P);
    fn set_parameter<P: Param>(&self, param: &P, value: P::Plain);
    fn end_set_parameter<P: Param>(&self, param: &P);

    /// Clicks, reset gestures and committed numeric edits are complete transactions.
    fn set_discrete_parameter<P: Param>(&self, param: &P, value: P::Plain) {
        self.begin_set_parameter(param);
        self.set_parameter(param, value);
        self.end_set_parameter(param);
    }
}

pub struct GestureSetter<'a> {
    setter: &'a ParamSetter<'a>,
    gestures: &'a ParameterGestures,
}
impl GestureSetter<'_> {
    pub fn begin_set_parameter<P: Param>(&self, param: &P) {
        let mut active = self.gestures.active.lock().unwrap();
        if active.contains(&param.as_ptr()) {
            return;
        }
        active.push(param.as_ptr());
        drop(active);
        self.setter.begin_set_parameter(param);
    }

    /// An explicit begin keeps a drag open across updates. Without one, this is
    /// a complete gesture for a click, keyboard edit, or committed text entry.
    pub fn set_parameter<P: Param>(&self, param: &P, value: P::Plain) {
        self.set_parameter_normalized(param, param.preview_normalized(value));
    }

    pub fn end_set_parameter<P: Param>(&self, param: &P) {
        let mut active = self.gestures.active.lock().unwrap();
        let Some(index) = active.iter().position(|ptr| *ptr == param.as_ptr()) else {
            return;
        };
        active.swap_remove(index);
        drop(active);
        self.setter.end_set_parameter(param);
    }
}

impl GestureSetter<'_> {
    pub fn set_parameter_normalized<P: Param>(&self, param: &P, value: f32) {
        let ongoing = self
            .gestures
            .active
            .lock()
            .unwrap()
            .contains(&param.as_ptr());
        if !ongoing {
            self.begin_set_parameter(param);
        }
        self.setter.set_parameter_normalized(param, value);
        if !ongoing {
            self.end_set_parameter(param);
        }
    }
}

impl ParameterWriter for GestureSetter<'_> {
    fn begin_set_parameter<P: Param>(&self, param: &P) {
        self.begin_set_parameter(param);
    }
    fn set_parameter<P: Param>(&self, param: &P, value: P::Plain) {
        self.set_parameter(param, value);
    }
    fn end_set_parameter<P: Param>(&self, param: &P) {
        self.end_set_parameter(param);
    }
}

#[cfg(test)]
mod tests;
