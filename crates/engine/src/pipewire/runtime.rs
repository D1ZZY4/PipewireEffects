use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pw::spa;
use spa::pod::Pod;
use spa::utils::Direction;

use pipewire_effects_core::{AudioEffect, AudioRing, ChainCommand, SharedMeters};

use crate::config::EngineConfig;
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, EventSink, StreamRole};
use crate::processor::{CaptureSource, RtProcessor};
use crate::stats::StreamStats;

use super::callbacks::{CaptureCallback, PlaybackCallback};
use super::props::{capture_properties, format_pod, playback_properties};
use super::{PUMP_INTERVAL, SCRATCH_FRAMES_PER_CHANNEL};

struct Runtime<'a> {
    config: &'a EngineConfig,
    commands: &'a Arc<ArrayQueue<ChainCommand>>,
    recycle: &'a Arc<ArrayQueue<Box<dyn AudioEffect>>>,
    ring: &'a Arc<AudioRing>,
    input_meters: &'a Arc<SharedMeters>,
    output_meters: &'a Arc<SharedMeters>,
    stats: &'a Arc<StreamStats>,
    events: &'a EventSink,
    stop: &'a Arc<AtomicBool>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    config: &EngineConfig,
    commands: &Arc<ArrayQueue<ChainCommand>>,
    recycle: &Arc<ArrayQueue<Box<dyn AudioEffect>>>,
    ring: &Arc<AudioRing>,
    input_meters: &Arc<SharedMeters>,
    output_meters: &Arc<SharedMeters>,
    stats: &Arc<StreamStats>,
    events: &EventSink,
    stop: &Arc<AtomicBool>,
) -> Result<()> {
    let runtime = Runtime {
        config,
        commands,
        recycle,
        ring,
        input_meters,
        output_meters,
        stats,
        events,
        stop,
    };
    run_streams(&runtime)
}

fn run_streams(runtime: &Runtime<'_>) -> Result<()> {
    let Runtime {
        config,
        commands,
        recycle,
        ring,
        input_meters,
        output_meters,
        stats,
        events,
        stop,
    } = runtime;

    let format = config.format;

    pw::init();
    let mainloop = pw::main_loop::MainLoopBox::new(None)?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None)?;
    let core = context.connect(None)?;

    let capture =
        pw::stream::StreamBox::new(&core, &config.capture_node_name, capture_properties(config))?;

    let capture_callback = CaptureCallback {
        source: CaptureSource::new(
            Arc::clone(ring),
            Arc::clone(input_meters),
            Arc::clone(stats),
            format,
        ),
        format,
        scratch: Vec::new(),
        role: StreamRole::Capture,
        events: Arc::clone(events),
    };

    let _capture_listener = capture
        .add_local_listener_with_user_data(capture_callback)
        .param_changed(|_, callback, id, param| callback.on_param_changed(id, param))
        .state_changed(|_, callback, _old, new| callback.on_state_changed(new))
        .process(|stream, callback| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let datas = buffer.datas_mut();
            let Some(slot) = datas.first_mut() else {
                return;
            };
            let valid = usize::try_from(slot.chunk().size()).unwrap_or(0);
            let Some(bytes) = slot.data() else {
                return;
            };
            let valid = valid.min(bytes.len());
            callback.capture(&bytes[..valid]);
        })
        .register()?;

    let capture_params = format_pod(format);
    let mut capture_pods = [Pod::from_bytes(&capture_params)
        .ok_or_else(|| EngineError::InvalidConfig(String::from("bad capture format pod")))?];

    capture.connect(
        Direction::Input,
        None,
        pw::stream::StreamFlags::MAP_BUFFERS | pw::stream::StreamFlags::RT_PROCESS,
        &mut capture_pods,
    )?;

    let mut playback_guard = None;
    if config.play_audio {
        let playback = pw::stream::StreamBox::new(
            &core,
            &config.playback_node_name,
            playback_properties(config),
        )?;

        let playback_callback = PlaybackCallback {
            processor: RtProcessor::new(
                format,
                Arc::clone(commands),
                Arc::clone(recycle),
                Arc::clone(ring),
                Arc::clone(output_meters),
                Arc::clone(stats),
                config.target_fill_samples(),
                config.target_fill_samples() * 2,
            ),
            scratch: vec![0.0; SCRATCH_FRAMES_PER_CHANNEL * format.channels_usize().max(1)],
            channels: format.channels_usize().max(1),
            role: StreamRole::Playback,
            events: Arc::clone(events),
        };

        let listener = playback
            .add_local_listener_with_user_data(playback_callback)
            .param_changed(|_, callback, id, param| callback.on_param_changed(id, param))
            .state_changed(|_, callback, _old, new| callback.on_state_changed(new))
            .process(|stream, callback| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let requested = usize::try_from(buffer.requested()).unwrap_or(0);

                let datas = buffer.datas_mut();
                let Some(slot) = datas.first_mut() else {
                    return;
                };

                let stride = (size_of::<f32>() * callback.channels) as u32;
                let frames = {
                    let Some(bytes) = slot.data() else {
                        return;
                    };
                    let capacity = bytes.len() / stride as usize;
                    let frames = if requested > 0 {
                        requested.min(capacity)
                    } else {
                        capacity
                    };
                    callback.render(bytes, frames);
                    frames
                };

                let chunk = slot.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = stride as _;
                *chunk.size_mut() = (stride as usize * frames) as _;
            })
            .register()?;

        let playback_params = format_pod(format);
        let mut playback_pods = [Pod::from_bytes(&playback_params)
            .ok_or_else(|| EngineError::InvalidConfig(String::from("bad playback format pod")))?];

        playback.connect(
            Direction::Output,
            None,
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS
                | pw::stream::StreamFlags::RT_PROCESS,
            &mut playback_pods,
        )?;

        playback_guard = Some((playback, listener));
    }

    events(EngineEvent::Started {
        capture_node: config.capture_node_name.clone(),
        format,
    });

    while !stop.load(Ordering::Relaxed) {
        mainloop
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(PUMP_INTERVAL));

        while recycle.pop().is_some() {}
    }

    drop(playback_guard);
    drop(_capture_listener);
    Ok(())
}
