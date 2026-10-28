mod callbacks;
mod props;
mod runtime;
mod samples;

use runtime::run;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pipewire_effects_core::{AudioFormat, AudioRing, ChainCommand, SharedMeters};
use pw::spa;
use spa::param::audio::AudioInfoRaw;
use spa::param::format::{MediaSubtype, MediaType};
use spa::param::format_utils;
use spa::pod::Pod;

use crate::config::EngineConfig;
use crate::error::{EngineError, Result};
use crate::events::{ConnectionState, EngineEvent, EventSink};
use crate::processor::recycle_queue;
use crate::stats::{StatsSnapshot, StreamStats};

const PUMP_INTERVAL: Duration = Duration::from_millis(50);

const SCRATCH_FRAMES_PER_CHANNEL: usize = 8_192;

pub struct Engine {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn spawn(
        config: EngineConfig,
        commands: Arc<ArrayQueue<ChainCommand>>,
        events: EventSink,
    ) -> Result<(Self, EngineHandles)> {
        config.validate().map_err(EngineError::InvalidConfig)?;

        let format = config.format;
        let ring = Arc::new(AudioRing::with_capacity(config.ring_capacity_samples()));
        let input_meters = SharedMeters::new(format.channels_usize());
        let output_meters = SharedMeters::new(format.channels_usize());
        let stats = StreamStats::new();
        let recycle = recycle_queue();

        let handles = EngineHandles {
            ring: Arc::clone(&ring),
            input_meters: Arc::clone(&input_meters),
            output_meters: Arc::clone(&output_meters),
            stats: Arc::clone(&stats),
            format,
        };

        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let thread = thread::Builder::new()
            .name(String::from("pipewire-effects"))
            .spawn(move || {
                if let Err(error) = run(
                    &config,
                    &commands,
                    &recycle,
                    &ring,
                    &input_meters,
                    &output_meters,
                    &stats,
                    &events,
                    &thread_stop,
                ) {
                    events(EngineEvent::Failed {
                        message: error.to_string(),
                    });
                } else {
                    events(EngineEvent::Stopped);
                }
            })
            .map_err(EngineError::ThreadSpawn)?;

        Ok((
            Self {
                stop,
                thread: Some(thread),
            },
            handles,
        ))
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("running", &self.is_running())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct EngineHandles {
    ring: Arc<AudioRing>,
    input_meters: Arc<SharedMeters>,
    output_meters: Arc<SharedMeters>,
    stats: Arc<StreamStats>,
    format: AudioFormat,
}

impl EngineHandles {
    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    #[must_use]
    pub fn input_meters(&self) -> &Arc<SharedMeters> {
        &self.input_meters
    }

    #[must_use]
    pub fn output_meters(&self) -> &Arc<SharedMeters> {
        &self.output_meters
    }

    #[must_use]
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    #[must_use]
    pub fn buffered_samples(&self) -> usize {
        self.ring.len()
    }

    #[must_use]
    pub fn latency_ms(&self) -> f64 {
        self.stats
            .snapshot()
            .latency_ms(self.format.sample_rate, self.format.channels)
    }

    pub fn reset_problems(&self) {
        self.stats.reset_problems();
    }
}

pub(super) fn connection_state(state: pw::stream::StreamState) -> ConnectionState {
    match state {
        pw::stream::StreamState::Unconnected => ConnectionState::Unconnected,
        pw::stream::StreamState::Connecting => ConnectionState::Connecting,
        pw::stream::StreamState::Paused => ConnectionState::Paused,
        pw::stream::StreamState::Streaming => ConnectionState::Streaming,
        pw::stream::StreamState::Error(_) => ConnectionState::Error,
    }
}

pub(super) fn negotiated_format(param: &Pod) -> Option<AudioFormat> {
    let (media_type, media_subtype) = format_utils::parse_format(param).ok()?;
    if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
        return None;
    }
    let mut info = AudioInfoRaw::new();
    info.parse(param).ok()?;
    let channels = u16::try_from(info.channels()).ok()?;
    let format = AudioFormat::new(info.rate(), channels);
    format.is_plausible().then_some(format)
}

#[cfg(test)]
mod tests;
