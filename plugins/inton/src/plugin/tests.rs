use super::*;

#[test]
fn host_and_core_parameter_defaults_agree() {
    let shared = Arc::new(Shared::new());
    let params = IntonParams::new(shared);
    assert_eq!(params.values(), Parameters::default());
}
#[test]
fn restoring_project_uses_applied_host_parameters_and_modulation() {
    use nice_plug::params::InternalParamMut;
    let shared = Arc::new(Shared::new());
    let params = IntonParams::new(shared.clone());
    let fields = params.serialize_fields();
    unsafe {
        params.reference._internal_set_plain_value(432.0);
        params.reference._internal_modulate_value(0.1);
    }
    params.deserialize_fields(&fields);
    assert_eq!(shared.parameters.read().unwrap(), params.values());
    assert!(shared.error.lock().unwrap().is_none());
}
#[test]
fn host_parameters_reach_the_worker_without_audio_or_an_editor() {
    use nice_plug::params::InternalParamMut;
    let shared = Arc::new(Shared::new());
    let params = IntonParams::new(shared.clone());
    unsafe {
        params.reference._internal_set_plain_value(432.0);
        params.enabled._internal_set_plain_value(false);
    }
    let (frequencies, _, _, enabled) = shared.publication(0.005);
    assert!((frequencies[69] - 432.0).abs() < 1e-8);
    assert!(!enabled);
}

#[test]
fn host_callbacks_route_each_parameter_to_its_core_field() {
    use nice_plug::params::InternalParamMut;
    let shared = Arc::new(Shared::new());
    let params = IntonParams::new(shared.clone());
    unsafe {
        params.position._internal_set_plain_value(3);
        params.morph_ms._internal_set_plain_value(250.0);
        params.reference._internal_set_plain_value(432.0);
        params.transpose._internal_set_plain_value(-7);
        params.enabled._internal_set_plain_value(false);
    }
    assert_eq!(
        shared.parameters.read(),
        Some(Parameters {
            position: 3,
            morph_ms: 250.0,
            reference: 432.0,
            transpose: -7,
            enabled: false,
        })
    );
}
