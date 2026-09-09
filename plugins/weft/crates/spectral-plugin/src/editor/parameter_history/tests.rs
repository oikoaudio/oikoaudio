use super::*;
use crate::parameters::SpectralParams;
use nice_plug::{
    context::{PluginApi, gui::GuiContextInner},
    prelude::PluginState,
};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Notification {
    Begin(ParamPtr),
    Set(ParamPtr, f32),
    End(ParamPtr),
}

#[derive(Default)]
struct DelayedHost(Mutex<Vec<Notification>>);

impl GuiContextInner for DelayedHost {
    fn request_restart(&self) {}

    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    unsafe fn raw_begin_set_parameter(&self, param: ParamPtr) {
        self.0.lock().unwrap().push(Notification::Begin(param));
    }
    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, value: f32) {
        self.0.lock().unwrap().push(Notification::Set(param, value));
    }
    unsafe fn raw_end_set_parameter(&self, param: ParamPtr) {
        self.0.lock().unwrap().push(Notification::End(param));
    }
    fn get_state(&self) -> PluginState {
        panic!("Must not read whole plugin state")
    }
    fn set_state(&self, _: PluginState) {
        panic!("Must not restore whole plugin state")
    }
}

#[test]
fn queued_parameter_edits_undo_and_redo_without_waiting_for_host_echo() {
    let params = SpectralParams::default();
    let host = DelayedHost::default();
    let setter = ParamSetter::new(&host);
    let gestures = ParameterGestures::default();
    let tracked = TrackedParamSetter::new(&setter, &params, &gestures);
    let param = &params.partials;
    let before = param.unmodulated_normalized_value();
    let after = param.preview_normalized(12);
    let ptr = param.as_ptr();
    tracked.begin_set_parameter(param);
    tracked.set_parameter(param, 12);
    tracked.end_set_parameter(param);
    assert_eq!(param.unmodulated_normalized_value(), before);
    let changes = gestures.take_completed();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].after, after);

    let mut history = super::super::CurveHistory::default();
    history.record_before(super::super::HistoryEntry::Parameters(changes));
    // Host values have deliberately not caught up, including during redo.
    let current =
        || super::super::CurveSnapshot::capture(&[0.0; super::super::MANUAL_MASK_POINTS], &params);
    let super::super::HistoryEntry::Parameters(undo) = history.undo(current(), &params).unwrap()
    else {
        panic!("parameter entry")
    };
    restore_parameters(&undo, &params, &setter);
    let super::super::HistoryEntry::Parameters(redo) = history.redo(current(), &params).unwrap()
    else {
        panic!("parameter entry")
    };
    restore_parameters(&redo, &params, &setter);
    assert_eq!(
        *host.0.lock().unwrap(),
        vec![
            Notification::Begin(ptr),
            Notification::Set(ptr, after),
            Notification::End(ptr),
            Notification::Begin(ptr),
            Notification::Set(ptr, before),
            Notification::End(ptr),
            Notification::Begin(ptr),
            Notification::Set(ptr, after),
            Notification::End(ptr),
        ]
    );
}

#[test]
fn stationary_drag_sends_no_repeated_values_or_extra_host_gestures() {
    let params = SpectralParams::default();
    let host = DelayedHost::default();
    let setter = ParamSetter::new(&host);
    let gestures = ParameterGestures::default();
    let tracked = TrackedParamSetter::new(&setter, &params, &gestures);
    let param = &params.partials;
    tracked.begin_set_parameter(param);
    for _ in 0..100 {
        tracked.set_parameter(param, 12);
    }
    assert!(gestures.take_completed().is_empty());
    tracked.end_set_parameter(param);
    assert_eq!(gestures.take_completed().len(), 1);
    let ptr = param.as_ptr();
    assert_eq!(
        *host.0.lock().unwrap(),
        vec![
            Notification::Begin(ptr),
            Notification::Set(ptr, param.preview_normalized(12)),
            Notification::End(ptr),
        ]
    );
}

#[test]
fn unchanged_gesture_has_no_host_notifications_or_local_entry() {
    let params = SpectralParams::default();
    let host = DelayedHost::default();
    let setter = ParamSetter::new(&host);
    let gestures = ParameterGestures::default();
    let tracked = TrackedParamSetter::new(&setter, &params, &gestures);
    let param = &params.partials;
    tracked.begin_set_parameter(param);
    tracked.set_parameter(param, param.unmodulated_plain_value());
    tracked.end_set_parameter(param);
    assert!(gestures.take_completed().is_empty());
    assert!(host.0.lock().unwrap().is_empty());
}

#[test]
fn closing_editor_finishes_an_open_host_gesture() {
    let params = SpectralParams::default();
    let host = DelayedHost::default();
    let setter = ParamSetter::new(&host);
    let gestures = ParameterGestures::default();
    let tracked = TrackedParamSetter::new(&setter, &params, &gestures);
    let param = &params.partials;
    tracked.begin_set_parameter(param);
    tracked.set_parameter(param, 12);
    gestures.finish(&params, &setter);
    gestures.finish(&params, &setter);
    assert!(!gestures.is_active());
    assert_eq!(gestures.take_completed().len(), 1);
    assert_eq!(host.0.lock().unwrap().len(), 3);
    assert_eq!(
        host.0.lock().unwrap().last(),
        Some(&Notification::End(param.as_ptr()))
    );
}
