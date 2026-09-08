use display_data::{DISPLAY_REFRESH_HZ, ModulationDisplay};
mod display_data;
mod editor;

use editor::{EDITOR_HEIGHT, EDITOR_WIDTH, WowEditor, closest_ui_scale};
use oiko_plugin::HostCoordinateEditor;
type UiScaleState = oiko_plugin::UiScaleState<100>;
#[cfg(test)]
use nice_plug::params::persist::PersistentField;
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditorState, EguiNiceSettings, RepaintNotifier, create_egui_editor};
use oiko_dsp::fractional_delay::{QualityMode, QualityVariableDelayF32, SincBankF32};
use std::{
    num::NonZeroU32,
    sync::{Arc, OnceLock},
};
use wow_dsp::modulation::{
    ModulationEngine, ModulationParams, maximum_delay_excursion_samples, maximum_rate_delta,
};

const MIN_RATE_HZ: f64 = 0.1;
const MIN_CONSTANT_PITCH_RATE_HZ: f64 = 0.2;
const MAX_RATE_HZ: f64 = 4.0;
const MAX_DEPTH_CENTS: f64 = 60.0;
const MIN_FLUTTER_RATE_HZ: f64 = 6.0;
const MAX_FLUTTER_RATE_HZ: f64 = 30.0;
const MAX_FLUTTER_DEPTH_CENTS: f64 = 20.0;
const TIME_DEPTH_SCALE: f64 = 4.0;
const KERNEL_MARGIN: usize = 68;
static KERNEL_BANK: OnceLock<Arc<SincBankF32>> = OnceLock::new();

pub struct WowPlugin {
    params: Arc<WowParams>,
    channels: Vec<Channel>,
    sample_rate: f64,
    modulation: ModulationEngine,
    base_delay_samples: usize,
    depth_behavior: PluginDepthBehavior,
    applied_delays: [Option<f64>; 2],
    display: Arc<ModulationDisplay>,
    display_counter: usize,
    display_interval_samples: usize,
    editor_state: Arc<EguiEditorState>,
    random_seed: u64,
}

struct Channel {
    wet: QualityVariableDelayF32,
}

#[derive(Params)]
struct WowParams {
    #[persist = "ui-scale-v1"]
    ui_scale: UiScaleState,

    #[id = "rate"]
    rate: FloatParam,

    #[id = "flutter_rate"]
    flutter_rate: FloatParam,

    #[id = "amount"]
    amount: FloatParam,

    #[id = "wow_flutter"]
    wow_flutter: FloatParam,

    // Preserve the original stable ID so existing automation survives the
    // user-facing rename from Flux to Drift.
    #[id = "flux"]
    drift: FloatParam,

    #[id = "stereo"]
    stereo: FloatParam,

    #[id = "random_seed"]
    random_seed: IntParam,

    #[id = "quality"]
    quality: EnumParam<PluginQuality>,

    #[id = "depth_behavior"]
    depth_behavior: EnumParam<PluginDepthBehavior>,
}

#[derive(Clone, Copy, Debug, Eq, Enum, PartialEq)]
enum PluginQuality {
    #[id = "draft"]
    Draft,
    #[id = "normal"]
    Normal,
    #[id = "hq"]
    #[name = "HQ"]
    Hq,
    #[id = "ultra"]
    Ultra,
}

#[derive(Clone, Copy, Debug, Default, Eq, Enum, PartialEq)]
enum PluginDepthBehavior {
    #[id = "time"]
    #[name = "Rate-scaled"]
    #[default]
    Time,
    #[id = "pitch"]
    #[name = "Constant"]
    Pitch,
}

impl From<PluginQuality> for QualityMode {
    fn from(value: PluginQuality) -> Self {
        match value {
            PluginQuality::Draft => Self::Draft,
            PluginQuality::Normal => Self::Normal,
            PluginQuality::Hq => Self::Hq,
            PluginQuality::Ultra => Self::Ultra,
        }
    }
}

impl Default for WowParams {
    fn default() -> Self {
        let wow_rate_range = FloatRange::Skewed {
            min: MIN_RATE_HZ as f32,
            max: MAX_RATE_HZ as f32,
            factor: skew_factor_for_midpoint(MIN_RATE_HZ as f32, 0.6, MAX_RATE_HZ as f32),
        };
        let flutter_rate_range = FloatRange::Skewed {
            min: MIN_FLUTTER_RATE_HZ as f32,
            max: MAX_FLUTTER_RATE_HZ as f32,
            factor: skew_factor_for_midpoint(
                MIN_FLUTTER_RATE_HZ as f32,
                12.0,
                MAX_FLUTTER_RATE_HZ as f32,
            ),
        };

        Self {
            ui_scale: UiScaleState::default(),
            rate: FloatParam::new("Wow Rate", 0.6, wow_rate_range)
                .with_smoother(SmoothingStyle::Logarithmic(50.0))
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            flutter_rate: FloatParam::new("Flutter Rate", 12.0, flutter_rate_range)
                .with_smoother(SmoothingStyle::Logarithmic(50.0))
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            amount: FloatParam::new("Amount", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0))
                .with_value_to_string(formatters::v2s_f32_percentage(1))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            wow_flutter: FloatParam::new(
                "Wow / Flutter",
                0.1,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(Arc::new(|value| {
                format!("{:.0}/{:.0}", (1.0 - value) * 100.0, value * 100.0)
            })),
            drift: FloatParam::new("Drift", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(100.0))
                .with_value_to_string(formatters::v2s_f32_percentage(1))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            stereo: FloatParam::new(
                "L/R Phase Offset",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(100.0))
            .with_value_to_string(Arc::new(|value| format!("{:.0}°", value * 180.0)))
            .with_string_to_value(Arc::new(|text| {
                text.trim()
                    .trim_end_matches('°')
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .map(|degrees| degrees / 180.0)
            })),
            random_seed: IntParam::new(
                "Random Seed",
                1,
                IntRange::Linear {
                    min: 0,
                    max: 65_535,
                },
            ),
            // A mode switch crossfades kernels against the same delay history,
            // so this can remain host-visible and automatable without clicks.
            quality: EnumParam::new("Quality", PluginQuality::Hq),
            // Changing the depth law also changes the required causal delay.
            // Hosts may restart processing when the reported latency changes.
            depth_behavior: EnumParam::new("Pitch Range", PluginDepthBehavior::Time)
                .non_automatable(),
        }
    }
}

impl Default for WowPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(WowParams::default()),
            channels: Vec::new(),
            sample_rate: 48_000.0,
            modulation: ModulationEngine::new(48_000.0, 1),
            base_delay_samples: 0,
            depth_behavior: PluginDepthBehavior::Time,
            applied_delays: [None; 2],
            display: Arc::new(ModulationDisplay::default()),
            display_counter: 0,
            display_interval_samples: 400,
            editor_state: EguiEditorState::from_size(
                nice_plug::editor::dpi::LogicalSize {
                    width: EDITOR_WIDTH,
                    height: EDITOR_HEIGHT,
                },
                1.0,
            ),
            random_seed: 1,
        }
    }
}

impl Plugin for WowPlugin {
    const NAME: &'static str = "Oiko Wow";
    const VENDOR: &'static str = "Oiko Audio";
    const URL: &'static str = "https://oikoaudio.com/wow/";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type Editor = HostCoordinateEditor<WowEditor>;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        // Seed geometry from the fixed canvas without compounding user zoom.
        // Hosts may create this editor before restoring state and reuse it on
        // reopen; WowEditor::build reconciles the size with restored zoom.
        let interface_scale = closest_ui_scale(self.params.ui_scale.get());
        self.params.ui_scale.set(interface_scale);
        self.editor_state =
            oiko_plugin::editor_state(egui::vec2(EDITOR_WIDTH, EDITOR_HEIGHT), interface_scale);
        let editor_state = self.editor_state.clone();
        create_egui_editor(
            self.editor_state.clone(),
            RepaintNotifier::new(),
            EguiNiceSettings::new().with_tile(Self::NAME),
            WowEditor::new(self.params.clone(), self.display.clone()),
        )
        .map(|editor| HostCoordinateEditor::new(editor, editor_state))
    }

    fn activate(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate as f64;
        let maximum_rate_delta = maximum_supported_rate_delta();
        let maximum_delay_excursion =
            maximum_delay_excursion(PluginDepthBehavior::Pitch, self.sample_rate);
        let maximum_base_delay = base_delay_samples(PluginDepthBehavior::Pitch, self.sample_rate);
        let max_delay = (maximum_base_delay as f64 + maximum_delay_excursion).ceil() as usize;
        self.depth_behavior = self.params.depth_behavior.value();
        self.base_delay_samples = base_delay_samples(self.depth_behavior, self.sample_rate);
        let bank = KERNEL_BANK
            .get_or_init(|| Arc::new(SincBankF32::for_max_rate(1.0 + maximum_rate_delta)))
            .clone();
        let quality = self.params.quality.value().into();
        let channels = audio_io_layout
            .main_output_channels
            .map(NonZeroU32::get)
            .unwrap_or(0) as usize;
        self.channels = (0..channels)
            .map(|_| Channel {
                wet: QualityVariableDelayF32::new(
                    max_delay,
                    KERNEL_MARGIN,
                    bank.clone(),
                    quality,
                    256,
                ),
            })
            .collect();
        self.random_seed = self.params.random_seed.value() as u64;
        self.modulation = ModulationEngine::new(self.sample_rate, self.random_seed);
        self.applied_delays = [None; 2];
        self.display_counter = 0;
        self.display_interval_samples =
            (self.sample_rate / DISPLAY_REFRESH_HZ).round().max(1.0) as usize;
        self.display.clear();
        context.set_latency_samples(self.base_delay_samples as u32);
        true
    }

    fn reset(&mut self) {
        self.modulation.reset();
        self.applied_delays = [None; 2];
        self.display_counter = 0;
        self.display.clear();
        for channel in &mut self.channels {
            channel.wet.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let depth_behavior = self.params.depth_behavior.value();
        if depth_behavior != self.depth_behavior {
            self.depth_behavior = depth_behavior;
            self.base_delay_samples = base_delay_samples(depth_behavior, self.sample_rate);
            context.set_latency_samples(self.base_delay_samples as u32);
        }
        let seed = self.params.random_seed.value() as u64;
        if seed != self.random_seed {
            self.random_seed = seed;
            self.modulation.set_seed(seed);
        }
        // The sinc bank covers every normal modulation trajectory. Host
        // automation can move a constant-pitch delay target much faster than
        // the LFO itself, especially close to the 0.2 Hz depth knee. Bound the
        // actual read-head motion so the rate-aware filter always receives a
        // playback rate it was designed to handle. This also turns a Pitch
        // Range latency change into a bounded transition if the host does not
        // restart processing immediately.
        let maximum_delay_step = maximum_supported_rate_delta();
        for frame in buffer.iter_samples() {
            let rate = self.params.rate.smoothed.next() as f64;
            let flutter_rate = self.params.flutter_rate.smoothed.next() as f64;
            let amount = self.params.amount.smoothed.next() as f64;
            let wow_flutter = self.params.wow_flutter.smoothed.next() as f64;
            let (depth_cents, flutter_depth) =
                modulation_depths(amount, wow_flutter, rate, flutter_rate, self.depth_behavior);
            let drift = self.params.drift.smoothed.next() as f64;
            let stereo_amount = self.params.stereo.smoothed.next() as f64;
            let quality = self.params.quality.value().into();
            let offset = self.modulation.next(ModulationParams {
                wow_rate_hz: rate,
                wow_depth_cents: depth_cents,
                wow_shape: 0.0,
                flutter_rate_hz: flutter_rate,
                flutter_depth_cents: flutter_depth,
                flutter_shape: 0.0,
                drift_amount: drift,
                stereo_amount,
            });

            let mut display_rates = [1.0_f64; 2];
            let mut processed_channels = 0;
            for (channel_index, (sample, channel)) in
                frame.into_iter().zip(&mut self.channels).enumerate()
            {
                let input = *sample;
                let channel_offset = if channel_index == 0 {
                    offset.left
                } else {
                    offset.right
                };
                let target_delay = self.base_delay_samples as f64 + channel_offset;
                let (delay, playback_rate) = bounded_delay_step(
                    self.applied_delays[channel_index],
                    target_delay,
                    maximum_delay_step,
                );
                self.applied_delays[channel_index] = Some(delay);
                display_rates[channel_index] = playback_rate;
                processed_channels += 1;
                let wet =
                    channel
                        .wet
                        .process_sample_rate_aware(input, delay, quality, playback_rate);
                *sample = wet;
            }
            if processed_channels == 1 {
                display_rates[1] = display_rates[0];
            }
            self.display_counter += 1;
            if self.display_counter >= self.display_interval_samples {
                self.display_counter = 0;
                self.display.push(
                    (display_rates[0] - 1.0) as f32,
                    (display_rates[1] - 1.0) as f32,
                );
            }
        }
        ProcessStatus::Normal
    }
}

#[inline]
fn modulation_depths(
    amount: f64,
    wow_flutter: f64,
    wow_rate_hz: f64,
    flutter_rate_hz: f64,
    behavior: PluginDepthBehavior,
) -> (f64, f64) {
    let theta = wow_flutter.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2;
    let amount = amount.clamp(0.0, 1.0);
    let wow_weight = amount * theta.cos();
    let flutter_weight = amount * theta.sin();
    match behavior {
        PluginDepthBehavior::Time => {
            let wow_rate_hz = wow_rate_hz.clamp(MIN_RATE_HZ, MAX_RATE_HZ);
            let flutter_rate_hz = flutter_rate_hz.clamp(MIN_FLUTTER_RATE_HZ, MAX_FLUTTER_RATE_HZ);
            let wow_reference_delta = speed_delta(MAX_DEPTH_CENTS);
            let flutter_reference_delta = speed_delta(MAX_FLUTTER_DEPTH_CENTS);

            // Preserve the constant-power delay-excursion budget of the rate-scaled
            // mode, but aim that budget so the balance knob describes the audible
            // pitch contribution in the same way as it does in constant mode.
            let delay_budget = (wow_reference_delta * TIME_DEPTH_SCALE * wow_weight / MAX_RATE_HZ)
                .hypot(
                    flutter_reference_delta * TIME_DEPTH_SCALE * flutter_weight
                        / MAX_FLUTTER_RATE_HZ,
                );
            let desired_wow_delay = speed_delta(MAX_DEPTH_CENTS * wow_weight) / wow_rate_hz;
            let desired_flutter_delay =
                speed_delta(MAX_FLUTTER_DEPTH_CENTS * flutter_weight) / flutter_rate_hz;
            let desired_delay = desired_wow_delay.hypot(desired_flutter_delay);
            let normalization = if desired_delay > f64::EPSILON {
                delay_budget / desired_delay
            } else {
                0.0
            };

            (
                cents_from_speed_delta(desired_wow_delay * normalization * wow_rate_hz),
                cents_from_speed_delta(desired_flutter_delay * normalization * flutter_rate_hz),
            )
        }
        PluginDepthBehavior::Pitch => (
            pitch_depth_with_rate_floor(
                MAX_DEPTH_CENTS * wow_weight,
                wow_rate_hz,
                MIN_CONSTANT_PITCH_RATE_HZ,
            ),
            MAX_FLUTTER_DEPTH_CENTS * flutter_weight,
        ),
    }
}

fn maximum_delay_excursion(behavior: PluginDepthBehavior, sample_rate: f64) -> f64 {
    let (wow_rate, flutter_rate) = match behavior {
        PluginDepthBehavior::Time => (MAX_RATE_HZ, MAX_FLUTTER_RATE_HZ),
        PluginDepthBehavior::Pitch => (MIN_CONSTANT_PITCH_RATE_HZ, MIN_FLUTTER_RATE_HZ),
    };
    let wow = maximum_delay_excursion_samples(MAX_DEPTH_CENTS, wow_rate, sample_rate);
    let flutter =
        maximum_delay_excursion_samples(MAX_FLUTTER_DEPTH_CENTS, flutter_rate, sample_rate);
    let scale = match behavior {
        PluginDepthBehavior::Time => TIME_DEPTH_SCALE,
        PluginDepthBehavior::Pitch => 1.0,
    };
    wow.hypot(flutter) * scale
}

fn base_delay_samples(behavior: PluginDepthBehavior, sample_rate: f64) -> usize {
    (maximum_delay_excursion(behavior, sample_rate) + KERNEL_MARGIN as f64 + 2.0).ceil() as usize
}

#[inline]
fn speed_delta(depth_cents: f64) -> f64 {
    2.0_f64.powf(depth_cents / 1200.0) - 1.0
}

#[inline]
fn cents_from_speed_delta(delta: f64) -> f64 {
    1200.0 * (1.0 + delta.max(0.0)).log2()
}

#[inline]
fn pitch_depth_with_rate_floor(depth_cents: f64, rate_hz: f64, floor_hz: f64) -> f64 {
    let below_floor_scale = (rate_hz / floor_hz).clamp(0.0, 1.0);
    cents_from_speed_delta(speed_delta(depth_cents) * below_floor_scale)
}

fn maximum_supported_rate_delta() -> f64 {
    let time_wow = cents_from_speed_delta(speed_delta(MAX_DEPTH_CENTS) * TIME_DEPTH_SCALE);
    let time_flutter =
        cents_from_speed_delta(speed_delta(MAX_FLUTTER_DEPTH_CENTS) * TIME_DEPTH_SCALE);
    maximum_rate_delta(&[time_wow, time_flutter])
}

#[inline]
fn bounded_delay_step(
    previous_delay: Option<f64>,
    target_delay: f64,
    maximum_step: f64,
) -> (f64, f64) {
    let Some(previous_delay) = previous_delay else {
        return (target_delay, 1.0);
    };
    let step = (target_delay - previous_delay).clamp(-maximum_step, maximum_step);
    (previous_delay + step, 1.0 - step)
}

fn skew_factor_for_midpoint(min: f32, midpoint: f32, max: f32) -> f32 {
    debug_assert!(min < midpoint && midpoint < max);
    0.5_f32.ln() / ((midpoint - min) / (max - min)).ln()
}

fn display_rate_scale(params: &WowParams) -> f32 {
    let (wow_depth, flutter_depth) = modulation_depths(
        1.0,
        params.wow_flutter.modulated_plain_value() as f64,
        params.rate.modulated_plain_value() as f64,
        params.flutter_rate.modulated_plain_value() as f64,
        params.depth_behavior.value(),
    );
    let drift_multiplier =
        2.0_f64.powf(0.5 * params.drift.modulated_plain_value().clamp(0.0, 1.0) as f64);
    ((speed_delta(wow_depth).abs() + speed_delta(flutter_depth).abs()) * drift_multiplier * 1.05)
        .max(5.0e-4) as f32
}

impl ClapPlugin for WowPlugin {
    const CLAP_ID: &'static str = "com.oikoaudio.wow";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Low-aliasing variable-speed pitch modulation");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://oikoaudio.com/wow/");
    const CLAP_SUPPORT_URL: Option<&'static str> =
        Some("https://github.com/oikoaudio/oikoaudio/issues");
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::PitchShifter,
    ];
}

impl Vst3Plugin for WowPlugin {
    // UUID 728eae36-7e0a-409a-9b92-eb5d9d591386. This is the plug-in's permanent
    // VST3 identity and must remain stable after the first public build.
    const VST3_CLASS_ID: [u8; 16] = [
        0x72, 0x8e, 0xae, 0x36, 0x7e, 0x0a, 0x40, 0x9a, 0x9b, 0x92, 0xeb, 0x5d, 0x9d, 0x59, 0x13,
        0x86,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Modulation];
}

nice_export_clap!(WowPlugin);
nice_export_vst3!(WowPlugin);

#[cfg(test)]
mod tests;
