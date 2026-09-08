use super::*;
use nice_plug::{
    context::{PluginApi, gui::GuiContextInner},
    prelude::*,
};

#[derive(Params)]
struct TestParams {
    #[id = "value"]
    value: FloatParam,
}
#[derive(Default)]
struct Host(Mutex<Vec<&'static str>>);
impl GuiContextInner for Host {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {
        self.0.lock().unwrap().push("begin");
    }
    unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {
        self.0.lock().unwrap().push("set");
    }
    unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {
        self.0.lock().unwrap().push("end");
    }
    fn get_state(&self) -> PluginState {
        panic!("Gesture must not read state")
    }
    fn set_state(&self, _: PluginState) {
        panic!("Gesture must not replace state")
    }
}
#[test]
fn drag_has_one_gesture_and_close_finishes_it_once() {
    let params = TestParams {
        value: FloatParam::new("Value", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
    };
    let host = Host::default();
    let setter = ParamSetter::new(&host);
    let gestures = ParameterGestures::default();
    let tracked = gestures.setter(&setter);
    tracked.begin_set_parameter(&params.value);
    tracked.begin_set_parameter(&params.value);
    tracked.set_parameter(&params.value, 0.2);
    tracked.set_parameter(&params.value, 0.4);
    gestures.finish(&params, &setter);
    gestures.finish(&params, &setter);
    tracked.end_set_parameter(&params.value);
    assert_eq!(*host.0.lock().unwrap(), ["begin", "set", "set", "end"]);
    tracked.set_parameter(&params.value, 0.5);
    assert_eq!(
        *host.0.lock().unwrap(),
        ["begin", "set", "set", "end", "begin", "set", "end"]
    );
}
