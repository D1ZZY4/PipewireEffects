use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pw::spa;
use pw::spa::pod::Pod;
use pw::stream::{StreamFlags, StreamListener, StreamRc};
use pwe_core::{ChainCommand, Effect};

use crate::config::{EngineConfig, GROUP_NAME, MAX_QUANTUM_FRAMES};
use crate::error::{EngineError, EngineResult};
use crate::processor::{CaptureState, Processor};
use crate::processors::{PlaybackState, StreamCapture};
use crate::shared::{EngineState, Shared};
use crate::streams::{audio_format_pod, parse_rate, stream_properties};

pub const CAPTURE_NODE: &str = "pipewire-effects-sink";
pub const PLAYBACK_NODE: &str = "pipewire-effects-output";

pub struct Streams {
    pub capture: StreamRc,
    pub playback: StreamRc,
    pub capture_listener: StreamListener<StreamCapture>,
    pub playback_listener: StreamListener<PlaybackState>,
}

pub struct StreamSetup {
    pub core: pw::core::CoreRc,
    pub config: EngineConfig,
    pub shared: Arc<Shared>,
    pub rate: Arc<AtomicU32>,
    pub target: String,
    pub bypassed: bool,
    pub commands: Arc<ArrayQueue<ChainCommand>>,
    pub retired: Arc<ArrayQueue<Box<dyn Effect>>>,
}

pub fn build(setup: StreamSetup) -> EngineResult<Streams> {
    let (producer, consumer) = rtrb::RingBuffer::new(setup.config.ring_capacity_samples());

    let (capture, capture_listener) = create_capture(&setup, producer)?;
    let (playback, playback_listener) = create_playback(&setup, consumer)?;
    Ok(Streams {
        capture,
        playback,
        capture_listener,
        playback_listener,
    })
}

fn create_capture(
    setup: &StreamSetup,
    producer: rtrb::Producer<f32>,
) -> EngineResult<(StreamRc, StreamListener<StreamCapture>)> {
    let mut properties = stream_properties(
        "Audio/Sink",
        "Capture",
        "DSP",
        &setup.config.sink_node_name,
        &setup.config.sink_description,
        GROUP_NAME,
    );
    properties.insert("node.virtual", "true");
    properties.insert("priority.session", "0");
    properties.insert("monitor.channel-volumes", "false");
    let stream = StreamRc::new(setup.core.clone(), CAPTURE_NODE, properties)?;

    let state = CaptureState::new(
        producer,
        Arc::clone(&setup.shared),
        Arc::clone(&setup.rate),
        setup.config.channels,
    );
    let rate = Arc::clone(&setup.rate);
    let shared = Arc::clone(&setup.shared);
    let channels = setup.config.channels;
    let listener = stream
        .add_local_listener_with_user_data(StreamCapture { state })
        .param_changed(move |_stream, _state, id, param| {
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            if let Some(parsed) = param.and_then(parse_rate) {
                rate.store(parsed, Ordering::Relaxed);
            }
        })
        .state_changed(move |_stream, _state, old, new| {
            report_state(&shared, "capture", old, new, channels);
        })
        .process(|stream, state| state.process(stream))
        .register()?;

    let pod = audio_format_pod(setup.config.channels)
        .ok_or(EngineError::StreamUnavailable("capture format"))?;
    let Some(parsed) = Pod::from_bytes(&pod) else {
        return Err(EngineError::StreamUnavailable("capture format"));
    };
    let mut params = [parsed];
    stream.connect(
        spa::utils::Direction::Input,
        None,
        StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
        &mut params,
    )?;
    Ok((stream, listener))
}

fn create_playback(
    setup: &StreamSetup,
    consumer: rtrb::Consumer<f32>,
) -> EngineResult<(StreamRc, StreamListener<PlaybackState>)> {
    let mut properties = stream_properties(
        "Stream/Output/Audio",
        "Playback",
        "DSP",
        PLAYBACK_NODE,
        "PipeWire Effects Output",
        GROUP_NAME,
    );
    if !setup.target.is_empty() {
        properties.insert("target.object", setup.target.as_str());
        properties.insert("node.target", setup.target.as_str());
    }
    let stream = StreamRc::new(setup.core.clone(), PLAYBACK_NODE, properties)?;

    let processor = Processor::new(
        Arc::clone(&setup.commands),
        Arc::clone(&setup.retired),
        Arc::clone(&setup.shared),
        Arc::clone(&setup.rate),
        setup.config.channels,
    );
    let mut processor = processor;
    processor.prepare(pwe_core::AudioFormat::new(
        setup.rate.load(Ordering::Relaxed).max(48_000),
        setup.config.channels,
    ));
    if setup.bypassed {
        let _ = setup.commands.push(ChainCommand::SetBypass(true));
    }
    let state = PlaybackState::new(
        processor,
        consumer,
        setup.config.channels,
        MAX_QUANTUM_FRAMES * setup.config.channels,
    );

    let rate = Arc::clone(&setup.rate);
    let shared = Arc::clone(&setup.shared);
    let channels = setup.config.channels;
    let listener = stream
        .add_local_listener_with_user_data(state)
        .param_changed(move |_stream, _state, id, param| {
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            if let Some(parsed) = param.and_then(parse_rate) {
                rate.store(parsed, Ordering::Relaxed);
            }
        })
        .state_changed(move |_stream, _state, old, new| {
            report_state(&shared, "playback", old, new, channels);
        })
        .process(|stream, state| state.process(stream))
        .register()?;

    let pod = audio_format_pod(setup.config.channels)
        .ok_or(EngineError::StreamUnavailable("playback format"))?;
    let Some(parsed) = Pod::from_bytes(&pod) else {
        return Err(EngineError::StreamUnavailable("playback format"));
    };
    let mut params = [parsed];
    stream.connect(
        spa::utils::Direction::Output,
        None,
        StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS | StreamFlags::AUTOCONNECT,
        &mut params,
    )?;
    Ok((stream, listener))
}

fn report_state(
    shared: &Shared,
    which: &'static str,
    old: pw::stream::StreamState,
    new: pw::stream::StreamState,
    channels: usize,
) {
    match new {
        pw::stream::StreamState::Error(message) => {
            shared.set_error(Some(format!("the {which} stream failed: {message}")));
            shared.set_state(EngineState::Failed);
        }
        pw::stream::StreamState::Streaming => {
            shared.set_error(None);
            shared.set_state(EngineState::Running);
            shared.update_snapshot(|snapshot| snapshot.channels = channels);
        }
        pw::stream::StreamState::Unconnected | pw::stream::StreamState::Paused => {
            shared.add_warning(format!("the {which} stream left {old:?} for {new:?}"));
        }
        _ => {}
    }
}
