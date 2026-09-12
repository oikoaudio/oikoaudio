use super::*;
use nice_plug::params::InternalParamMut;

struct Context {
    transport: Transport,
}
impl ActivateContext<WowPlugin> for Context {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute(&self, _: ()) {}
    fn set_latency_samples(&self, _: u32) {}
    fn set_current_voice_capacity(&self, _: u32) {}
}
impl ProcessContext<WowPlugin> for Context {
    fn request_restart(&self) {}
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute_background(&self, _: ()) {}
    fn execute_gui(&self, _: ()) {}
    fn transport(&self) -> &Transport {
        &self.transport
    }
    fn next_event(&mut self) -> Option<NoteEvent<()>> {
        None
    }
    fn try_send_event(
        &mut self,
        event: NoteEvent<()>,
    ) -> Result<(), (NoteEvent<()>, nice_plug::context::process::SendEventError)> {
        let _ = event;
        Ok(())
    }
    fn set_latency_samples(&self, _: u32) {}
    fn set_current_voice_capacity(&self, _: u32) {}
}

fn setup(sync_wow: bool, sync_flutter: bool) -> (WowPlugin, Context) {
    let mut plugin = WowPlugin::default();
    let mut context = Context {
        transport: Transport::new(48_000.0),
    };
    context.transport.tempo = Some(120.0);
    unsafe {
        plugin.params.rate._internal_set_plain_value(1.0);
        plugin.params.flutter_rate._internal_set_plain_value(12.0);
        plugin.params.rate_sync._internal_set_plain_value(sync_wow);
        plugin
            .params
            .flutter_rate_sync
            ._internal_set_plain_value(sync_flutter);
        plugin
            .params
            .rate_division
            ._internal_set_plain_value(RateDivision::Half);
        plugin
            .params
            .flutter_rate_division
            ._internal_set_plain_value(RateDivision::SixteenthTriplet);
    }
    plugin.params.rate.smoothed.reset(1.0);
    plugin.params.flutter_rate.smoothed.reset(12.0);
    plugin.params.amount.smoothed.reset(0.8);
    plugin.params.wow_flutter.smoothed.reset(0.4);
    plugin.params.drift.smoothed.reset(0.3);
    plugin.params.stereo.smoothed.reset(0.25);
    assert!(plugin.activate(
        &WowPlugin::AUDIO_IO_LAYOUTS[0],
        &BufferConfig {
            sample_rate: 48_000.0,
            min_buffer_size: None,
            max_buffer_size: 512,
            process_mode: ProcessMode::Realtime,
        },
        &mut context
    ));
    plugin.reset();
    (plugin, context)
}

fn process(
    plugin: &mut WowPlugin,
    context: &mut Context,
    start: usize,
    samples: usize,
) -> Vec<f32> {
    let mut audio: [Vec<f32>; 2] = std::array::from_fn(|_| {
        (start..start + samples)
            .map(|n| (n as f32 * 0.061).sin() * 0.1)
            .collect()
    });
    let mut buffer = Buffer::default();
    unsafe {
        buffer.set_slices(samples, |slices| {
            *slices = audio.iter_mut().map(|v| v.as_mut_slice()).collect()
        });
    }
    nice_assert_no_alloc::assert_no_alloc(|| {
        plugin.process(
            &mut buffer,
            &mut AuxiliaryBuffers {
                inputs: &mut [],
                outputs: &mut [],
            },
            context,
        );
    });
    assert!(audio.iter().flatten().all(|v| v.is_finite()));
    audio.into_iter().flatten().collect()
}

#[test]
fn independent_sync_modes_match_equivalent_hz_processing() {
    for (wow, flutter) in [(false, true), (true, false), (true, true)] {
        let (mut synced, mut context) = setup(wow, flutter);
        let (mut free, mut free_context) = setup(false, false);
        let wow_hz = if wow { 2.0 } else { 1.0 };
        let flutter_hz = if flutter { 24.0 } else { 12.0 };
        unsafe {
            synced
                .params
                .rate_division
                ._internal_set_plain_value(RateDivision::Quarter);
            synced
                .params
                .flutter_rate_division
                ._internal_set_plain_value(RateDivision::ThirtySecondTriplet);
            free.params.rate._internal_set_plain_value(wow_hz);
            free.params
                .flutter_rate
                ._internal_set_plain_value(flutter_hz);
        }
        free.params.rate.smoothed.reset(wow_hz);
        free.params.flutter_rate.smoothed.reset(flutter_hz);
        for start in (0..4096).step_by(256) {
            assert_eq!(
                process(&mut synced, &mut context, start, 256),
                process(&mut free, &mut free_context, start, 256)
            );
        }
    }
}

#[test]
fn returning_to_hz_after_changing_divisions_preserves_the_audible_speed() {
    let (mut switched, mut context) = setup(true, true);
    let (mut reference, mut reference_context) = setup(true, true);
    for plugin in [&mut switched, &mut reference] {
        unsafe {
            plugin
                .params
                .rate_division
                ._internal_set_plain_value(RateDivision::Eighth);
            plugin
                .params
                .flutter_rate_division
                ._internal_set_plain_value(RateDivision::ThirtySecondTriplet);
        }
    }
    assert_eq!(
        process(&mut switched, &mut context, 0, 512),
        process(&mut reference, &mut reference_context, 0, 512)
    );
    unsafe {
        switched.params.rate._internal_set_plain_value(4.0);
        switched.params.flutter_rate._internal_set_plain_value(24.0);
        switched.params.rate_sync._internal_set_plain_value(false);
        switched
            .params
            .flutter_rate_sync
            ._internal_set_plain_value(false);
    }
    switched.params.rate.smoothed.set_target(48_000.0, 4.0);
    switched
        .params
        .flutter_rate
        .smoothed
        .set_target(48_000.0, 24.0);
    for start in (512..4096).step_by(512) {
        assert_eq!(
            process(&mut switched, &mut context, start, 512),
            process(&mut reference, &mut reference_context, start, 512)
        );
    }
}

#[test]
fn tempo_and_mode_transitions_are_bounded_and_allocation_free() {
    let (mut plugin, mut context) = setup(true, true);
    for (step, tempo) in [
        Some(90.0),
        Some(143.0),
        None,
        Some(f64::NAN),
        Some(1.0),
        Some(960.0),
    ]
    .into_iter()
    .enumerate()
    {
        context.transport.tempo = tempo;
        context.transport.playing = step % 2 == 0;
        context.transport.pos_beats = Some(if step % 2 == 0 { -8.0 } else { 1024.0 });
        unsafe {
            plugin
                .params
                .rate_sync
                ._internal_set_plain_value(step % 2 == 0);
            plugin
                .params
                .flutter_rate_sync
                ._internal_set_plain_value(step % 2 != 0);
            plugin
                .params
                .depth_behavior
                ._internal_set_plain_value(if step % 2 == 0 {
                    PluginDepthBehavior::Time
                } else {
                    PluginDepthBehavior::Pitch
                });
        }
        process(&mut plugin, &mut context, step * 512, 512);
        if step == 2 || step == 3 {
            assert_eq!(plugin.tempo_bpm, 143.0);
        }
    }
    nice_assert_no_alloc::assert_no_alloc(|| plugin.reset());
}

#[test]
fn synchronized_output_is_independent_of_host_block_size_and_playhead_jumps() {
    let render = |block: usize| {
        let (mut plugin, mut context) = setup(true, true);
        let mut output = Vec::new();
        for (start, tempo) in [(0, 120.0), (1024, 90.0), (2048, 143.0)] {
            context.transport.tempo = Some(tempo);
            for offset in (0..1024).step_by(block) {
                context.transport.playing = offset % 2 == 0;
                context.transport.pos_beats = Some(offset as f64 - 100.0);
                let audio = process(&mut plugin, &mut context, start + offset, block);
                output.extend_from_slice(&audio[..block]);
            }
        }
        output
    };
    let expected = render(512);
    for block in [1, 16, 128] {
        assert_eq!(render(block), expected);
    }
}
