#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pw::spa;
use pw::spa::pod::Pod;
use pw::stream::{StreamFlags, StreamListener, StreamRc};
use pwe_core::i18n::tr;
use pwe_core::{ChainCommand, Effect, PipelineKind};

use super::stream_spec::{self, NodeSpec};
use crate::capture::CaptureState;
use crate::config::{EngineConfig, MAX_QUANTUM_FRAMES};
use crate::cpu::Processor;
use crate::error::{EngineError, EngineResult};
use crate::processors::{PlaybackState, StreamCapture};
use crate::shared::{EngineState, Shared};
use crate::streams::{audio_format_pod, parse_rate};

pub struct Streams {
    pub upstream: StreamRc,
    pub downstream: StreamRc,
    pub upstream_listener: StreamListener<StreamCapture>,
    pub downstream_listener: StreamListener<PlaybackState>,
    pub kind: PipelineKind,
}

impl Streams {
    pub fn virtual_node_id(&self) -> u32 {
        match self.kind {
            PipelineKind::Output => self.upstream.node_id(),
            PipelineKind::Input => self.downstream.node_id(),
        }
    }
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
    let upstream = stream_spec::upstream(&setup.config, &setup.target);
    let downstream = stream_spec::downstream(&setup.config, &setup.target);

    let (up, upstream_listener) = create_upstream(&setup, upstream, producer)?;
    let (down, downstream_listener) = create_downstream(&setup, downstream, consumer)?;
    Ok(Streams {
        upstream: up,
        downstream: down,
        upstream_listener,
        downstream_listener,
        kind: setup.config.kind,
    })
}

fn create_upstream(
    setup: &StreamSetup,
    spec: NodeSpec,
    producer: rtrb::Producer<f32>,
) -> EngineResult<(StreamRc, StreamListener<StreamCapture>)> {
    let flags = spec.flags();
    let stream = StreamRc::new(
        setup.core.clone(),
        spec.node_name.as_str(),
        spec.properties(),
    )?;

    let state = CaptureState::new(
        producer,
        Arc::clone(&setup.shared),
        Arc::clone(&setup.rate),
        setup.config.channels,
    );
    let rate = Arc::clone(&setup.rate);
    let shared = Arc::clone(&setup.shared);
    let channels = setup.config.channels;
    let side = setup.config.kind;
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
            report_state(&shared, side, "source", old, new, channels);
        })
        .process(|stream, state| state.process(stream))
        .register()?;

    connect(
        &stream,
        spa::utils::Direction::Input,
        setup.config.channels,
        flags,
    )?;
    Ok((stream, listener))
}

fn create_downstream(
    setup: &StreamSetup,
    spec: NodeSpec,
    consumer: rtrb::Consumer<f32>,
) -> EngineResult<(StreamRc, StreamListener<PlaybackState>)> {
    let flags = spec.flags();
    let stream = StreamRc::new(
        setup.core.clone(),
        spec.node_name.as_str(),
        spec.properties(),
    )?;

    let mut processor = Processor::new(
        Arc::clone(&setup.commands),
        Arc::clone(&setup.retired),
        Arc::clone(&setup.shared),
        Arc::clone(&setup.rate),
        setup.config.channels,
        setup.config.backend,
    );
    if let Some(reference) = &setup.config.reference {
        processor.attach_reference(setup.config.kind, Arc::clone(reference));
    }
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
    let side = setup.config.kind;
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
            report_state(&shared, side, "sink", old, new, channels);
        })
        .process(|stream, state| state.process(stream))
        .register()?;

    connect(
        &stream,
        spa::utils::Direction::Output,
        setup.config.channels,
        flags,
    )?;
    Ok((stream, listener))
}

fn connect(
    stream: &StreamRc,
    direction: spa::utils::Direction,
    channels: usize,
    flags: StreamFlags,
) -> EngineResult<()> {
    let pod = audio_format_pod(channels).ok_or(EngineError::StreamUnavailable("format"))?;
    let Some(parsed) = Pod::from_bytes(&pod) else {
        return Err(EngineError::StreamUnavailable("format"));
    };
    let mut params = [parsed];
    stream.connect(direction, None, flags, &mut params)?;
    Ok(())
}

fn report_state(
    shared: &Shared,
    kind: PipelineKind,
    which: &'static str,
    old: pw::stream::StreamState,
    new: pw::stream::StreamState,
    channels: usize,
) {
    match new {
        pw::stream::StreamState::Error(message) => {
            shared.set_error(Some(tr(
                "the {0} {1} stream failed: {2}",
                &[kind.noun(), which, &message],
            )));
            shared.set_state(EngineState::Failed);
        }
        pw::stream::StreamState::Streaming => {
            shared.set_error(None);
            shared.set_state(EngineState::Running);
            shared.update_snapshot(|snapshot| snapshot.channels = channels);
        }
        pw::stream::StreamState::Unconnected | pw::stream::StreamState::Paused => {
            if matches!(old, pw::stream::StreamState::Streaming) {
                shared.add_warning(tr(
                    "the {0} {1} stream left {2} for {3}",
                    &[kind.noun(), which, &format!("{old:?}"), &format!("{new:?}")],
                ));
            }
        }
        _ => {}
    }
}
