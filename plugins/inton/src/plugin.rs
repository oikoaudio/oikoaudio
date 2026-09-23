//! NICE-PLUG format integration. Inton's tuning engine remains format-independent.

use crate::{
    editor::Editor,
    editor_model::ViewPreferences,
    library::{Preferences, preferences_path},
};
use inton_core::engine::Parameter;
use inton_core::{
    engine::Parameters,
    runtime::Shared,
    state::{Project, ValidatedProject},
};
use nice_plug::context::gui::GuiContext;
use nice_plug::{params::internals::ParamPtr, plugin::ParamValue, prelude::*};
use nice_plug_egui::{
    EguiEditor, EguiEditorState, EguiNiceSettings, RepaintNotifier, create_egui_editor,
};
use std::collections::BTreeMap;
use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex, atomic::Ordering::SeqCst},
    thread::JoinHandle,
};

pub const PARAM_NAMES: [&str; 5] = [
    "Set position",
    "Morph Time",
    "Reference Frequency",
    "Transpose",
    "MTS Master Enabled",
];
pub const PARAM_IDS: [&str; 5] = [
    "set_position",
    "morph_time",
    "reference_frequency",
    "transpose",
    "mts_enabled",
];
pub const MIN: [f64; 5] = [0., 0., 400., -24., 0.];
pub const MAX: [f64; 5] = [31., 10000., 480., 24., 1.];

pub struct IntonParams {
    pub(crate) position: IntParam,
    pub(crate) morph_ms: FloatParam,
    pub(crate) reference: FloatParam,
    pub(crate) transpose: IntParam,
    pub(crate) enabled: BoolParam,
    shared: Arc<Shared>,
}

// The map contains only fields owned by this Params object. NicePlug retains
// the Arc returned by Plugin::params for the lifetime of those pointers.
unsafe impl Params for IntonParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        let pointers = [
            self.position.as_ptr(),
            self.morph_ms.as_ptr(),
            self.reference.as_ptr(),
            self.transpose.as_ptr(),
            self.enabled.as_ptr(),
        ];
        PARAM_IDS
            .into_iter()
            .zip(pointers)
            .map(|(id, pointer)| (id.to_owned(), pointer, String::new()))
            .collect()
    }

    fn serialize_fields(&self) -> BTreeMap<String, String> {
        BTreeMap::from([(
            "inton-project-v1".into(),
            serde_json::to_string(&self.shared.snapshot())
                .expect("Validated project is serializable"),
        )])
    }

    fn deserialize_fields(&self, fields: &BTreeMap<String, String>) {
        // NicePlug preflights the full state before setting any parameters.
        // Decode again here so direct callers also use the validated load path.
        let result = fields
            .get("inton-project-v1")
            .ok_or_else(|| "Missing Inton project".to_owned())
            .and_then(|encoded| ValidatedProject::decode(encoded.as_bytes()))
            .and_then(|mut project| {
                // NicePlug has now applied the host parameters, including modulation.
                // They are authoritative over the project's saved parameter copy.
                project.set_parameters(self.values());
                self.shared.restore_prepared(project)
            });
        if let Err(error) = result {
            *self.shared.error.lock().unwrap() = Some(error);
        }
    }
}

impl IntonParams {
    fn new(shared: Arc<Shared>) -> Self {
        let label_shared = shared.clone();
        let position_shared = shared.clone();
        let morph_shared = shared.clone();
        let reference_shared = shared.clone();
        let transpose_shared = shared.clone();
        let enabled_shared = shared.clone();
        Self {
            position: IntParam::new("Set position", 0, IntRange::Linear { min: 0, max: 31 })
                .with_callback(Arc::new(move |value| {
                    position_shared.set_parameter_value(Parameter::Position, value as f64);
                }))
                .with_value_to_string(Arc::new(move |value| {
                    label_shared
                        .project
                        .lock()
                        .unwrap()
                        .project
                        .label(value as usize)
                }))
                .with_string_to_value(Arc::new(|text| {
                    text.split_once(':')
                        .map_or(text, |(number, _)| number)
                        .trim()
                        .parse::<i32>()
                        .ok()
                        .map(|value| value - 1)
                })),
            morph_ms: FloatParam::new(
                "Morph Time",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10_000.0,
                },
            )
            .with_callback(Arc::new(move |value| {
                morph_shared.set_parameter_value(Parameter::MorphTime, value as f64);
            }))
            .with_unit(" ms")
            .with_value_to_string(Arc::new(|value| format!("{value:.1}")))
            .with_string_to_value(Arc::new(parse_number)),
            reference: FloatParam::new(
                "Reference Frequency",
                440.0,
                FloatRange::Linear {
                    min: 400.0,
                    max: 480.0,
                },
            )
            .with_callback(Arc::new(move |value| {
                reference_shared.set_parameter_value(Parameter::Reference, value as f64);
            }))
            .with_unit(" Hz")
            .with_value_to_string(Arc::new(|value| format!("{value:.3}")))
            .with_string_to_value(Arc::new(parse_number)),
            transpose: IntParam::new("Transpose", 0, IntRange::Linear { min: -24, max: 24 })
                .with_callback(Arc::new(move |value| {
                    transpose_shared.set_parameter_value(Parameter::Transpose, value as f64);
                }))
                .with_unit(" st")
                .with_value_to_string(Arc::new(|value| format!("{value:+}")))
                .with_string_to_value(Arc::new(|text| {
                    text.split_whitespace().next()?.parse().ok()
                })),
            enabled: BoolParam::new("MTS Master Enabled", true)
                .with_callback(Arc::new(move |value| {
                    enabled_shared.set_parameter_value(Parameter::Enabled, value as u8 as f64);
                }))
                .with_value_to_string(Arc::new(|enabled| {
                    if enabled { "Enabled" } else { "Disabled" }.into()
                }))
                .with_string_to_value(Arc::new(|text| match text.trim().to_lowercase().as_str() {
                    "enabled" | "on" | "1" => Some(true),
                    "disabled" | "off" | "0" => Some(false),
                    _ => None,
                })),
            shared,
        }
    }

    fn values(&self) -> Parameters {
        Parameters::from_values([
            self.position.value() as f64,
            self.morph_ms.value() as f64,
            self.reference.value() as f64,
            self.transpose.value() as f64,
            self.enabled.value() as u8 as f64,
        ])
    }
}

fn parse_number(text: &str) -> Option<f32> {
    text.split_whitespace().next()?.parse().ok()
}

struct EditorBridge {
    shared: Arc<Shared>,
    params: Arc<IntonParams>,
    context: Mutex<Option<GuiContext>>,
    gestures: oiko_plugin::gestures::ParameterGestures,
}

#[derive(Clone)]
pub struct HostRef {
    bridge: Option<Arc<EditorBridge>>,
}

impl HostRef {
    /// Create an editor host handle for standalone previews and tests.
    pub fn disconnected() -> Self {
        Self { bridge: None }
    }

    fn connected(shared: Arc<Shared>, params: Arc<IntonParams>) -> Self {
        Self {
            bridge: Some(Arc::new(EditorBridge {
                shared,
                params,
                context: Mutex::new(None),
                gestures: Default::default(),
            })),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.bridge.is_some()
    }

    pub fn attach(&self, context: GuiContext) {
        if let Some(bridge) = &self.bridge {
            *bridge.context.lock().unwrap() = Some(context);
        }
    }

    pub fn begin_parameter(&self, id: Parameter) {
        let Some(bridge) = &self.bridge else { return };
        let context = bridge.context.lock().unwrap().clone();
        let Some(context) = context else { return };
        let setter = context.param_setter();
        let tracked = bridge.gestures.setter(&setter);
        match id {
            Parameter::Position => tracked.begin_set_parameter(&bridge.params.position),
            Parameter::MorphTime => tracked.begin_set_parameter(&bridge.params.morph_ms),
            Parameter::Reference => tracked.begin_set_parameter(&bridge.params.reference),
            Parameter::Transpose => tracked.begin_set_parameter(&bridge.params.transpose),
            Parameter::Enabled => tracked.begin_set_parameter(&bridge.params.enabled),
        }
    }

    pub fn finish_gestures(&self) {
        let Some(bridge) = &self.bridge else { return };
        let context = bridge.context.lock().unwrap().clone();
        if let Some(context) = context {
            bridge
                .gestures
                .finish(&*bridge.params, &context.param_setter());
        }
    }

    pub fn detach(&self) {
        self.request_callback();
        self.finish_gestures();
        if let Some(bridge) = &self.bridge {
            *bridge.context.lock().unwrap() = None;
        }
    }

    pub fn request_callback(&self) {
        let Some(bridge) = &self.bridge else { return };
        let context = bridge.context.lock().unwrap().clone();
        let Some(context) = context else { return };
        let setter = context.param_setter();
        let tracked = bridge.gestures.setter(&setter);
        for (id, value) in bridge.shared.take_parameter_edits() {
            match id {
                Parameter::Position => tracked.set_parameter(&bridge.params.position, value as i32),
                Parameter::MorphTime => {
                    tracked.set_parameter(&bridge.params.morph_ms, value as f32)
                }
                Parameter::Reference => {
                    tracked.set_parameter(&bridge.params.reference, value as f32)
                }
                Parameter::Transpose => {
                    tracked.set_parameter(&bridge.params.transpose, value as i32)
                }
                Parameter::Enabled => tracked.set_parameter(&bridge.params.enabled, value >= 0.5),
            }
        }
    }
}

pub struct IntonPlugin {
    shared: Arc<Shared>,
    params: Arc<IntonParams>,
    worker: Option<JoinHandle<()>>,
    editor_state: Arc<EguiEditorState>,
}

impl Default for IntonPlugin {
    fn default() -> Self {
        let shared = Arc::new(Shared::new());
        let params = Arc::new(IntonParams::new(shared.clone()));
        let (editor_state, worker) = initial_state(&shared);
        Self {
            shared,
            params,
            worker,
            editor_state,
        }
    }
}

fn initial_state(shared: &Arc<Shared>) -> (Arc<EguiEditorState>, Option<JoinHandle<()>>) {
    let view = Preferences::load_from(&preferences_path())
        .unwrap_or_default()
        .view;
    let scale = ViewPreferences::nearest_scale(view.scale) as f32;
    let editor_state =
        oiko_plugin::editor_state(egui::vec2(view.width() as f32, view.height() as f32), scale);
    let worker = match shared.start() {
        Ok(worker) => Some(worker),
        Err(error) => {
            *shared.error.lock().unwrap() = Some(error.to_string());
            None
        }
    };
    (editor_state, worker)
}

impl Drop for IntonPlugin {
    fn drop(&mut self) {
        self.shared.stop.store(true, SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Plugin for IntonPlugin {
    const NAME: &'static str = "Oiko Inton";
    const VENDOR: &'static str = "Oiko Audio";
    const URL: &'static str = "https://oikoaudio.com/inton/";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type Editor = EguiEditor<Editor>;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn validate_state(state: &PluginState) -> Result<(), String> {
        let project = state
            .fields
            .get("inton-project-v1")
            .ok_or("Missing Inton project")?;
        Project::decode(project.as_bytes())?;
        for id in Parameter::ALL {
            let name = PARAM_IDS[id.index()];
            let value = match (id, state.params.get(name)) {
                (Parameter::Position | Parameter::Transpose, Some(ParamValue::I32(value))) => {
                    *value as f64
                }
                (Parameter::MorphTime | Parameter::Reference, Some(ParamValue::F32(value))) => {
                    *value as f64
                }
                (Parameter::Enabled, Some(ParamValue::Bool(_))) => continue,
                _ => return Err(format!("Missing or invalid parameter: {name}")),
            };
            if !value.is_finite() || !(MIN[id.index()]..=MAX[id.index()]).contains(&value) {
                return Err(format!("Parameter out of range: {name}"));
            }
        }
        Ok(())
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _: AsyncExecutor<Self>) -> Option<Self::Editor> {
        let view = Preferences::load_from(&preferences_path())
            .unwrap_or_default()
            .view;
        let scale = ViewPreferences::nearest_scale(view.scale) as f32;
        self.editor_state =
            oiko_plugin::editor_state(egui::vec2(view.width() as f32, view.height() as f32), scale);
        let host = HostRef::connected(self.shared.clone(), self.params.clone());
        create_egui_editor(
            self.editor_state.clone(),
            RepaintNotifier::new(),
            EguiNiceSettings::new().with_tile(Self::NAME),
            Editor::new(self.shared.clone(), host),
        )
    }

    fn process(
        &mut self,
        _: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        while let Some(event) = context.next_event() {
            let _ = context.try_send_event(event);
        }
        ProcessStatus::Normal
    }
}

impl ClapPlugin for IntonPlugin {
    const CLAP_ID: &'static str = "audio.oiko.inton";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Microtonal library, project Set and MTS-ESP tuning source");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Utility,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for IntonPlugin {
    // Published VST3 class ID. Never change it: hosts use it to find saved instances.
    const VST3_CLASS_ID: [u8; 16] = [
        0x5b, 0x4e, 0x8d, 0x4b, 0xe8, 0x85, 0x47, 0x64, 0xa6, 0xf8, 0x2e, 0xcb, 0xd8, 0x43, 0x0f,
        0x68,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nice_export_clap!(IntonPlugin);
nice_export_vst3!(IntonPlugin);

#[cfg(test)]
mod tests;
