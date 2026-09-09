use super::*;
use crate::SpectralPlugin;
use nice_plug::prelude::Plugin;
#[test]
fn old_and_new_shape_states_validate_and_bad_state_is_rejected_atomically() {
    let mut state = PluginState {
        version: "0.4.0-beta.1".into(),
        params: Default::default(),
        fields: Default::default(),
    };
    for index in 0..8 {
        state
            .params
            .insert("motion_shape".into(), ParamValue::I32(index));
        assert!(SpectralPlugin::validate_state(&state).is_ok());
    }
    state
        .params
        .insert("motion_shape".into(), ParamValue::I32(99));
    assert!(SpectralPlugin::validate_state(&state).is_err());
    state
        .params
        .insert("motion_shape".into(), ParamValue::I32(6));
    state
        .params
        .insert("motion_depth".into(), ParamValue::F32(f32::NAN));
    assert!(SpectralPlugin::validate_state(&state).is_err());
    state.params.remove("motion_depth");
    state
        .fields
        .insert("manual-curve-v1".into(), "[0,broken]".into());
    assert!(SpectralPlugin::validate_state(&state).is_err());
    state
        .fields
        .insert("manual-curve-v1".into(), "[0,-144]".into());
    assert!(SpectralPlugin::validate_state(&state).is_ok());
}
