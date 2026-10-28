//! The PipeWire backend.
//!
//! # Topology
//!
//! Two `pw_stream`s in one process, joined into a single PipeWire node group:
//!
//! ```text
//!   application ──► [capture stream: media.class=Audio/Sink]
//!                         │ process callback
//!                         ▼
//!                    ring buffer  (lock-free, absorbs jitter and drift)
//!                         │
//!                         ▼
//!                   effect chain  (runs on the playback thread)
//!                         │
//!                         ▼
//!   real device  ◄── [playback stream: media.class=Stream/Output/Audio]
//! ```
//!
//! # Why two streams and not one `pw_filter`
//!
//! `pw_filter` is the node type built for this job: it has both input and output
//! ports in a single node, so a chain of filters can be wired together with
//! explicit links and no buffer anywhere in the path. That is the better
//! topology — no jitter to absorb, no drift to correct, and cleaner integration
//! with the session manager because the virtual device is a real node rather
//! than a stream that happens to claim `Audio/Sink`.
//!
//! `pipewire-rs` does not bind `pw_filter` (as of 0.10), and writing the
//! bindings by hand would mean a large amount of `unsafe` around callbacks that
//! run on the realtime thread. The two-stream loopback is the approach
//! PipeWire's own `pw-loopback` tool takes, it uses only the safe API, and the
//! cost — a small buffer and a drift correction — is bounded and measurable.
//! The correction is counted in [`StreamStats`], so it is visible rather than
//! merely audible.
//!
//! # Why the format is fixed
//!
//! Both streams ask for exactly one format. PipeWire inserts a converter if the
//! hardware runs at something else, which means the negotiated format is always
//! the configured one. That in turn means `prepare` — the only method on
//! [`AudioEffect`] allowed to allocate — never has to run on the audio thread.

use std::mem::{align_of, size_of};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pipewire_effects_core::{AudioEffect, AudioFormat, AudioRing, ChainCommand, SharedMeters};
use pw::properties::properties;
use pw::spa;
use spa::param::ParamType;
use spa::param::audio::AudioInfoRaw;
use spa::param::format::{MediaSubtype, MediaType};
use spa::param::format_utils;
use spa::pod::Pod;
use spa::utils::Direction;

use crate::config::{EngineConfig, NODE_GROUP};
use crate::error::{EngineError, Result};
use crate::events::{ConnectionState, EngineEvent, EventSink, StreamRole};
use crate::processor::{CaptureSource, RtProcessor, recycle_queue};
use crate::stats::{StatsSnapshot, StreamStats};

/// How often the engine's own thread wakes up to check whether it should stop,
/// and to dispose of any retired effects.
const PUMP_INTERVAL: Duration = Duration::from_millis(50);

/// Samples the playback scratch buffer can hold, per channel.
///
/// A PipeWire quantum is normally 1024 frames; this is eight times that, so the
/// buffer never needs to grow while audio is running.
const SCRATCH_FRAMES_PER_CHANNEL: usize = 8_192;

/// A running engine.
///
/// Dropping the value stops the engine.
pub struct Engine {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// Starts the engine on a dedicated thread.
    ///
    /// `commands` is the queue the interface posts structural edits to; the
    /// engine drains it from the audio thread.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is unusable or the thread cannot
    /// be created. Problems that happen *after* startup are reported through
    /// `events` instead, because by then the caller has already moved on.
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

    /// Asks the engine to stop and waits for its thread.
    ///
    /// Safe to call more than once, and called automatically on drop.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // The engine wakes up on its own interval, so this returns promptly
            // even when the PipeWire server has gone away.
            let _ = thread.join();
        }
    }

    /// Whether the engine thread is still running.
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

/// The shared state the interface observes while the engine runs.
#[derive(Debug, Clone)]
pub struct EngineHandles {
    ring: Arc<AudioRing>,
    input_meters: Arc<SharedMeters>,
    output_meters: Arc<SharedMeters>,
    stats: Arc<StreamStats>,
    format: AudioFormat,
}

impl EngineHandles {
    /// The format the engine was configured for.
    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    /// Levels of the audio arriving from applications, before processing.
    #[must_use]
    pub fn input_meters(&self) -> &Arc<SharedMeters> {
        &self.input_meters
    }

    /// Levels of the audio being played, after processing.
    #[must_use]
    pub fn output_meters(&self) -> &Arc<SharedMeters> {
        &self.output_meters
    }

    /// The current counters.
    #[must_use]
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// Samples currently waiting in the ring buffer.
    #[must_use]
    pub fn buffered_samples(&self) -> usize {
        self.ring.len()
    }

    /// The current buffer occupancy in milliseconds.
    #[must_use]
    pub fn latency_ms(&self) -> f64 {
        self.stats
            .snapshot()
            .latency_ms(self.format.sample_rate, self.format.channels)
    }

    /// Clears the counters that describe problems, keeping the totals.
    pub fn reset_problems(&self) {
        self.stats.reset_problems();
    }
}

/// Everything the engine's thread needs, bundled so the signature stays sane.
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
fn run(
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

    // ---- Capture: the virtual sink applications play into ---------------
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
            // For an incoming buffer `chunk.size()` is the number of valid
            // bytes; the rest of the allocation is padding.
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

    // Deliberately *not* `AUTOCONNECT`. This node is a sink: PipeWire must not
    // go looking for a source to feed it. Its input ports exist for the
    // applications that choose to route here.
    capture.connect(
        Direction::Input,
        None,
        pw::stream::StreamFlags::MAP_BUFFERS | pw::stream::StreamFlags::RT_PROCESS,
        &mut capture_pods,
    )?;

    // ---- Playback: the processed audio, on its way to a real device ----
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
                // Trim at twice the target. That leaves room for normal jitter
                // without letting latency creep up unnoticed.
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
                // `requested` is the number of frames the graph actually wants.
                // Older servers leave it at zero, in which case we fill the
                // whole buffer, which is what PipeWire's own examples do.
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

    // The engine drives its own loop rather than calling `MainLoop::run`, so
    // that a stop request is honoured without needing a raw pointer to the
    // main loop from another thread.
    while !stop.load(Ordering::Relaxed) {
        mainloop
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(PUMP_INTERVAL));

        // Disposal happens here, never on the audio thread. Any thread that is
        // not realtime will do; this one is always running.
        while recycle.pop().is_some() {}
    }

    drop(playback_guard);
    drop(_capture_listener);
    Ok(())
}

/// The capture stream's properties.
fn capture_properties(config: &EngineConfig) -> pw::properties::PropertiesBox {
    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CLASS => "Audio/Sink",
        *pw::keys::MEDIA_ROLE => "DSP",
        *pw::keys::NODE_NAME => config.capture_node_name.clone(),
        *pw::keys::NODE_DESCRIPTION => config.capture_node_description.clone(),
        *pw::keys::NODE_VIRTUAL => "true",
        *pw::keys::NODE_GROUP => NODE_GROUP,
        *pw::keys::AUDIO_CHANNELS => config.format.channels.to_string(),
    };
    // Not exposed as a constant by pipewire-rs, but it is what makes PipeWire
    // treat the two streams as one scheduled unit.
    props.insert("node.link-group", NODE_GROUP);
    props
}

/// The playback stream's properties.
fn playback_properties(config: &EngineConfig) -> pw::properties::PropertiesBox {
    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Playback",
        *pw::keys::MEDIA_ROLE => "DSP",
        *pw::keys::NODE_NAME => config.playback_node_name.clone(),
        *pw::keys::NODE_GROUP => NODE_GROUP,
        *pw::keys::AUDIO_CHANNELS => config.format.channels.to_string(),
    };
    if let Some(target) = &config.target_sink {
        props.insert(*pw::keys::TARGET_OBJECT, target.clone());
    }
    props.insert("node.link-group", NODE_GROUP);
    props
}

/// Serialises the single format we will accept.
///
/// Asking for exactly one format is what keeps `prepare` off the audio thread;
/// see the module documentation.
fn format_pod(format: AudioFormat) -> Vec<u8> {
    let mut info = AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::F32LE);
    info.set_rate(format.sample_rate);
    info.set_channels(u32::from(format.channels));

    // Only stereo and mono have a position layout we can state confidently;
    // for anything else we let PipeWire pick a default.
    if format.channels == 2 {
        let mut positions = [0_u32; spa::sys::SPA_AUDIO_MAX_CHANNELS as usize];
        positions[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
        positions[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
        info.set_position(positions);
    }

    let object = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };

    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .map_or_else(|_| Vec::new(), |(cursor, _)| cursor.into_inner())
}

fn connection_state(state: pw::stream::StreamState) -> ConnectionState {
    match state {
        pw::stream::StreamState::Unconnected => ConnectionState::Unconnected,
        pw::stream::StreamState::Connecting => ConnectionState::Connecting,
        pw::stream::StreamState::Paused => ConnectionState::Paused,
        pw::stream::StreamState::Streaming => ConnectionState::Streaming,
        pw::stream::StreamState::Error(_) => ConnectionState::Error,
    }
}

/// Reads the negotiated format out of a `param_changed` parameter.
fn negotiated_format(param: &Pod) -> Option<AudioFormat> {
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

/// The capture stream's user data.
struct CaptureCallback {
    source: CaptureSource,
    format: AudioFormat,
    scratch: Vec<f32>,
    role: StreamRole,
    events: EventSink,
}

impl CaptureCallback {
    fn on_param_changed(&mut self, id: u32, param: Option<&Pod>) {
        if id != ParamType::Format.as_raw() {
            return;
        }
        let Some(param) = param else {
            return;
        };
        if let Some(format) = negotiated_format(param) {
            self.events(EngineEvent::FormatNegotiated {
                role: self.role,
                format,
            });
        }
    }

    fn on_state_changed(&self, state: pw::stream::StreamState) {
        self.events(EngineEvent::ConnectionChanged {
            role: self.role,
            state: connection_state(state),
        });
    }

    fn events(&self, event: EngineEvent) {
        (self.events)(event);
    }

    /// Buffers a block of samples that arrived from the graph.
    fn capture(&mut self, bytes: &[u8]) {
        let channels = self.format.channels_usize().max(1);
        let samples = read_samples(bytes, &mut self.scratch, channels);
        self.source.capture(samples);
    }
}

/// The playback stream's user data.
struct PlaybackCallback {
    processor: RtProcessor,
    scratch: Vec<f32>,
    channels: usize,
    role: StreamRole,
    events: EventSink,
}

impl PlaybackCallback {
    fn on_param_changed(&mut self, id: u32, param: Option<&Pod>) {
        if id != ParamType::Format.as_raw() {
            return;
        }
        let Some(param) = param else {
            return;
        };
        if let Some(format) = negotiated_format(param) {
            self.events(EngineEvent::FormatNegotiated {
                role: self.role,
                format,
            });
        }
    }

    fn on_state_changed(&self, state: pw::stream::StreamState) {
        self.events(EngineEvent::ConnectionChanged {
            role: self.role,
            state: connection_state(state),
        });
    }

    fn events(&self, event: EngineEvent) {
        (self.events)(event);
    }

    /// Fills an outgoing buffer with processed audio.
    fn render(&mut self, bytes: &mut [u8], frames: usize) {
        let frames = frames.min(self.scratch.len() / self.channels);
        let needed = frames * self.channels;
        if needed == 0 {
            bytes.fill(0);
            return;
        }
        self.processor.process(&mut self.scratch[..needed], frames);
        write_samples(&self.scratch[..needed], bytes);
    }
}

/// Whether a byte buffer can be viewed as `f32` samples without a copy.
///
/// Requires little-endian byte order, because we negotiate `F32LE`, and the
/// alignment `f32` needs. Mapped PipeWire buffers satisfy both in practice, but
/// checking costs nothing next to the risk of an unaligned load.
fn is_directly_viewable(pointer: *const u8) -> bool {
    cfg!(target_endian = "little") && (pointer as usize).is_multiple_of(align_of::<f32>())
}

/// Reads interleaved `f32` samples out of a PipeWire byte buffer.
///
/// Returns a slice of whichever input was actually used, so the caller does not
/// need to know whether a copy happened. `scratch` grows on first use and is
/// reused thereafter, so the steady state does not allocate.
fn read_samples<'a>(bytes: &'a [u8], scratch: &'a mut Vec<f32>, channels: usize) -> &'a [f32] {
    let channels = channels.max(1);
    let count = (bytes.len() / size_of::<f32>()) / channels * channels;

    if is_directly_viewable(bytes.as_ptr()) {
        // SAFETY: the pointer is checked to be correctly aligned for `f32`
        // above, and every bit pattern is a valid `f32`. The slice is bounded
        // by the length of `bytes`.
        return unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<f32>(), count) };
    }

    if scratch.len() < count {
        scratch.resize(count, 0.0);
    }
    for (slot, chunk) in scratch[..count].iter_mut().zip(bytes.chunks_exact(4)) {
        *slot = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    &scratch[..count]
}

/// Writes interleaved `f32` samples into a PipeWire byte buffer, little-endian.
fn write_samples(samples: &[f32], bytes: &mut [u8]) {
    let count = samples.len().min(bytes.len() / size_of::<f32>());
    if count == 0 {
        return;
    }
    let samples = &samples[..count];

    if is_directly_viewable(bytes.as_ptr()) {
        // SAFETY: alignment and endianness checked above, and the length is
        // bounded by the destination buffer.
        let destination =
            unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<f32>(), count) };
        destination.copy_from_slice(samples);
        return;
    }

    for (chunk, sample) in bytes.chunks_exact_mut(4).zip(samples) {
        chunk.copy_from_slice(&sample.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_pods_serialise_for_every_supported_channel_count() {
        for channels in [1_u16, 2, 6] {
            let format = AudioFormat::new(48_000, channels);
            let bytes = format_pod(format);
            assert!(!bytes.is_empty(), "no pod produced for {channels} channels");
            let pod = Pod::from_bytes(&bytes).expect("pod should parse");
            assert_eq!(negotiated_format(pod), Some(format));
        }
    }

    #[test]
    fn a_round_trip_through_the_byte_buffer_preserves_samples() {
        let samples = [0.0_f32, 0.25, -0.5, 1.0, -1.0, 3.5];
        let mut bytes = vec![0_u8; samples.len() * 4];
        write_samples(&samples, &mut bytes);

        let mut scratch = Vec::new();
        let read = read_samples(&bytes, &mut scratch, 2);
        assert_eq!(read, &samples);
    }

    #[test]
    fn a_misaligned_buffer_still_round_trips() {
        // Offset the destination by one byte so the fast path is unavailable.
        let samples = [0.125_f32, -0.25, 0.5, -1.0];
        let mut backing = vec![0_u8; samples.len() * 4 + 1];
        let bytes = &mut backing[1..];
        write_samples(&samples, bytes);

        let mut scratch = Vec::new();
        let read = read_samples(bytes, &mut scratch, 2);
        assert_eq!(read, &samples);
    }

    #[test]
    fn reading_ignores_a_trailing_partial_frame() {
        let bytes = vec![0_u8; 4 * 5]; // five samples, not a whole stereo frame
        let mut scratch = Vec::new();
        let read = read_samples(&bytes, &mut scratch, 2);
        assert_eq!(read.len(), 4, "a partial frame must not be exposed");
    }

    #[test]
    fn reading_a_buffer_of_whole_frames_keeps_all_of_them() {
        let bytes = vec![0_u8; 4 * 6];
        let mut scratch = Vec::new();
        assert_eq!(read_samples(&bytes, &mut scratch, 2).len(), 6);
    }

    #[test]
    fn scratch_reuse_does_not_leak_between_reads() {
        let mut scratch = Vec::new();
        let loud = vec![0xFF_u8; 4 * 4];
        assert_eq!(read_samples(&loud, &mut scratch, 2).len(), 4);

        let quiet = vec![0_u8; 4 * 2];
        let read = read_samples(&quiet, &mut scratch, 2);
        assert_eq!(read.len(), 2);
        assert!(read.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn direct_viewing_is_allowed_only_when_alignment_permits() {
        let aligned = [0_f32; 4];
        assert_eq!(
            is_directly_viewable(aligned.as_ptr().cast::<u8>()),
            cfg!(target_endian = "little")
        );

        let backing = [0_u8; 8];
        #[allow(clippy::cast_ptr_alignment)]
        let misaligned = unsafe { backing.as_ptr().add(1) };
        assert!(!is_directly_viewable(misaligned));
    }

    #[test]
    fn write_samples_never_writes_past_the_destination() {
        let samples = [1.0_f32; 8];
        let mut bytes = vec![0_u8; 4 * 4];
        write_samples(&samples, &mut bytes);
        let mut scratch = Vec::new();
        assert_eq!(read_samples(&bytes, &mut scratch, 1).len(), 4);
    }

    #[test]
    fn connection_states_map_one_to_one() {
        use pw::stream::StreamState;
        assert_eq!(
            connection_state(StreamState::Streaming),
            ConnectionState::Streaming
        );
        assert_eq!(
            connection_state(StreamState::Paused),
            ConnectionState::Paused
        );
        assert_eq!(
            connection_state(StreamState::Connecting),
            ConnectionState::Connecting
        );
        assert_eq!(
            connection_state(StreamState::Unconnected),
            ConnectionState::Unconnected
        );
        assert_eq!(
            connection_state(StreamState::Error(String::new())),
            ConnectionState::Error
        );
    }

    #[test]
    fn capture_properties_describe_a_virtual_sink() {
        let config = EngineConfig::default();
        let props = capture_properties(&config);
        assert_eq!(props.get("media.class"), Some("Audio/Sink"));
        assert_eq!(props.get("node.virtual"), Some("true"));
        assert_eq!(
            props.get("node.name"),
            Some(config.capture_node_name.as_str())
        );
        assert_eq!(props.get("node.link-group"), Some(NODE_GROUP));
    }

    #[test]
    fn playback_properties_target_the_configured_device() {
        let config = EngineConfig {
            target_sink: Some(String::from("alsa_output.test")),
            ..EngineConfig::default()
        };
        let props = playback_properties(&config);
        assert_eq!(props.get("target.object"), Some("alsa_output.test"));
        assert_eq!(props.get("media.category"), Some("Playback"));
    }

    #[test]
    fn playback_properties_omit_the_target_when_none_is_set() {
        let props = playback_properties(&EngineConfig::default());
        assert_eq!(props.get("target.object"), None);
    }
}
