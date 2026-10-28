//! The realtime processing core.
//!
//! Two objects own the audio path:
//!
//! * [`CaptureSource`] runs on the capture stream's callback. It meters the
//!   incoming audio and pushes it into the ring buffer.
//! * [`RtProcessor`] runs on the playback stream's callback. It drains pending
//!   structural edits, pulls audio out of the ring buffer, runs the effect
//!   chain over it, and publishes output meters.
//!
//! Splitting it this way is not incidental. The playback stream is driven by
//! the hardware clock, which is the stable one, so that is where the DSP runs.
//! The capture stream is driven by whatever is playing into the virtual sink,
//! which is far less predictable.
//!
//! # Why there is a ring buffer at all
//!
//! The two streams are separate nodes in the PipeWire graph and are scheduled
//! independently, so their callbacks are not in step. A small buffer absorbs
//! the jitter. That introduces a drift problem, since the two clocks are close
//! but not identical: [`RtProcessor`] therefore watches how full the buffer is
//! and periodically drops a sliver of audio to keep latency bounded. Both
//! corrections are counted in [`StreamStats`], so a system that is quietly
//! glitching is visible instead of merely sounding slightly off.
//!
//! # Realtime rules
//!
//! Nothing in this file allocates or locks. Effects retired by a structural edit
//! are pushed onto a recycle queue for the interface thread to drop. If that
//! queue is full the instance is dropped here, which is a fallback rather than a
//! design, and is counted.

use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use pipewire_effects_core::{
    AudioBlock, AudioEffect, AudioFormat, AudioRing, BlockMeter, ChainCommand, EffectChain,
    SharedMeters,
};

use crate::stats::StreamStats;

/// How many structural edits are applied in one audio callback.
///
/// Each edit moves a `Box`, so this is really a bound on how long the callback
/// can be held up. Edits arrive at human speed, so the limit is never reached in
/// practice; it exists so that a pathological burst cannot stall an interrupt.
const MAX_COMMANDS_PER_BLOCK: usize = 16;

/// How much room the recycle queue has for effects awaiting disposal.
const RECYCLE_CAPACITY: usize = 256;

/// What one pass through [`RtProcessor::process`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessorReport {
    /// Frames the caller asked to be filled.
    pub frames: usize,
    /// Samples actually taken from the ring buffer.
    pub sampled: usize,
    /// Samples that had to be invented as silence because the buffer ran dry.
    pub underrun_samples: usize,
    /// Samples discarded to correct accumulated latency.
    pub trimmed_samples: usize,
    /// Structural edits applied.
    pub commands_applied: usize,
}

impl ProcessorReport {
    /// Whether the pass completed without needing any correction.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.underrun_samples == 0 && self.trimmed_samples == 0
    }

    /// Whether the ring buffer had everything that was asked for.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.underrun_samples == 0
    }
}

/// The playback side of the engine: drains commands, runs the chain, meters.
pub struct RtProcessor {
    chain: EffectChain,
    commands: Arc<ArrayQueue<ChainCommand>>,
    recycle: Arc<ArrayQueue<Box<dyn AudioEffect>>>,
    ring: Arc<AudioRing>,
    output_meters: Arc<SharedMeters>,
    stats: Arc<StreamStats>,
    meter: BlockMeter,
    format: AudioFormat,
    channels: usize,
    target_fill: usize,
    high_watermark: usize,
}

impl RtProcessor {
    /// Builds a processor.
    ///
    /// `target_fill` is the number of samples the ring should hold in normal
    /// operation, and `high_watermark` is where accumulated latency gets
    /// trimmed back. Both are in samples, not frames.
    ///
    /// The argument list is long because the processor is assembled from
    /// pieces the engine already owns. Wrapping it in a builder would only move
    /// the same fields one indirection away.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        format: AudioFormat,
        commands: Arc<ArrayQueue<ChainCommand>>,
        recycle: Arc<ArrayQueue<Box<dyn AudioEffect>>>,
        ring: Arc<AudioRing>,
        output_meters: Arc<SharedMeters>,
        stats: Arc<StreamStats>,
        target_fill: usize,
        high_watermark: usize,
    ) -> Self {
        let channels = format.channels_usize().max(1);
        Self {
            chain: EffectChain::new(format),
            commands,
            recycle,
            ring,
            output_meters,
            stats,
            meter: BlockMeter::new(channels),
            format,
            channels,
            target_fill,
            high_watermark: high_watermark.max(target_fill + 1),
        }
    }

    /// The chain being processed. Exposed for tests and diagnostics.
    #[must_use]
    pub fn chain(&self) -> &EffectChain {
        &self.chain
    }

    /// The queue retired effects are pushed onto.
    #[must_use]
    pub fn recycle_queue(&self) -> &Arc<ArrayQueue<Box<dyn AudioEffect>>> {
        &self.recycle
    }

    /// Replaces the format and re-prepares the chain.
    ///
    /// Must be called from a thread that is not processing audio.
    pub fn set_format(&mut self, format: AudioFormat) {
        self.format = format;
        self.channels = format.channels_usize().max(1);
        self.chain.set_format(format);
        self.meter = BlockMeter::new(self.channels);
    }

    /// Applies pending structural edits.
    ///
    /// Returns how many were applied. Also called at the top of
    /// [`RtProcessor::process`], so the only reason to call it directly is a
    /// test or a diagnostic.
    pub fn drain_commands(&mut self) -> usize {
        let Self {
            chain,
            commands,
            recycle,
            stats,
            ..
        } = self;

        let mut applied = 0;
        while applied < MAX_COMMANDS_PER_BLOCK {
            let Some(command) = commands.pop() else {
                break;
            };
            command.apply(chain, &mut |effect| {
                if recycle.push(effect).is_err() {
                    // The interface thread is not draining the recycle queue.
                    // Dropping the instance here is the fallback, and it is at
                    // least bounded to one allocation.
                    stats.add_retired_drop();
                }
            });
            applied += 1;
        }
        applied
    }

    /// Fills `buffer` with processed audio.
    ///
    /// `frames` is clamped so that `frames * channels` never exceeds the
    /// buffer. This is the function called from the realtime callback.
    pub fn process(&mut self, buffer: &mut [f32], frames: usize) -> ProcessorReport {
        let channels = self.channels;
        let frames = frames.min(buffer.len() / channels);
        let needed = frames * channels;

        let mut report = ProcessorReport {
            frames,
            commands_applied: self.drain_commands(),
            ..ProcessorReport::default()
        };

        // Drift correction happens *before* reading, and only here: the read
        // cursor belongs to this side, so this is the only place it can be moved
        // safely.
        let buffered = self.ring.len();
        if buffered > self.high_watermark {
            let excess = buffered - self.target_fill;
            let trimmed = self.ring.discard_oldest(excess);
            self.stats.add_trim(trimmed as u64);
            report.trimmed_samples = trimmed;
        }

        let read = self.ring.read(&mut buffer[..needed]);
        report.sampled = read.moved;
        if read.shortfall > 0 {
            // Pad with silence rather than repeating a sample. A short gap is
            // far less noticeable than a click.
            buffer[read.moved..needed].fill(0.0);
            self.stats.add_underrun_samples(read.shortfall as u64);
            report.underrun_samples = read.shortfall;
        }

        if frames > 0 {
            let mut block = AudioBlock::with_frames(&mut buffer[..needed], self.format, frames);
            self.chain.process(&mut block);
            // A single non-finite sample anywhere upstream would otherwise
            // spread through the chain and silence the output permanently.
            block.sanitise();
            self.meter.accumulate_block(&block);
        }
        self.meter.publish(&self.output_meters);

        self.stats
            .add_processed_frames(u64::try_from(frames).unwrap_or(u64::MAX));
        self.stats
            .set_latency_samples(u64::try_from(self.ring.len()).unwrap_or(u64::MAX));
        report
    }
}

impl std::fmt::Debug for RtProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RtProcessor")
            .field("format", &self.format)
            .field("chain_len", &self.chain.len())
            .field("buffered_samples", &self.ring.len())
            .field("target_fill", &self.target_fill)
            .finish_non_exhaustive()
    }
}

/// The capture side: meters what arrives and buffers it for the processor.
pub struct CaptureSource {
    ring: Arc<AudioRing>,
    input_meters: Arc<SharedMeters>,
    stats: Arc<StreamStats>,
    meter: BlockMeter,
    channels: usize,
}

impl CaptureSource {
    /// Builds a capture source for a format.
    #[must_use]
    pub fn new(
        ring: Arc<AudioRing>,
        input_meters: Arc<SharedMeters>,
        stats: Arc<StreamStats>,
        format: AudioFormat,
    ) -> Self {
        let channels = format.channels_usize().max(1);
        Self {
            ring,
            input_meters,
            stats,
            meter: BlockMeter::new(channels),
            channels,
        }
    }

    /// Buffers one block of interleaved samples and updates the input meters.
    ///
    /// Returns how many samples were buffered. Samples that do not fit are
    /// dropped and counted: the alternative would be to overwrite audio the
    /// playback side has not consumed yet, which would be worse than a gap.
    pub fn capture(&mut self, samples: &[f32]) -> usize {
        let channels = self.channels;
        let frames = samples.len() / channels;
        let samples = &samples[..frames * channels];

        let report = self.ring.write(samples);
        if report.shortfall > 0 {
            self.stats.add_overrun_samples(report.shortfall as u64);
        }
        self.stats
            .add_capture_frames(u64::try_from(frames).unwrap_or(u64::MAX));

        self.meter.accumulate(samples, channels);
        self.meter.publish(&self.input_meters);

        report.moved
    }
}

impl std::fmt::Debug for CaptureSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureSource")
            .field("channels", &self.channels)
            .field("buffered_samples", &self.ring.len())
            .finish_non_exhaustive()
    }
}

/// Creates the recycle queue both sides share.
#[must_use]
pub fn recycle_queue() -> Arc<ArrayQueue<Box<dyn AudioEffect>>> {
    Arc::new(ArrayQueue::new(RECYCLE_CAPACITY))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipewire_effects_core::{
        EffectCategory, EffectDef, EffectKind, ParamSet, ParamSpec, ParamUnit,
    };

    struct Recorder {
        params: ParamSet,
    }

    impl Recorder {
        fn new() -> Self {
            Self {
                params: ParamSet::new(vec![ParamSpec::float(
                    "gain",
                    "Gain",
                    0.0,
                    4.0,
                    1.0,
                    0.1,
                    ParamUnit::Ratio,
                )]),
            }
        }
    }

    impl AudioEffect for Recorder {
        fn kind(&self) -> EffectKind {
            EffectKind::new("test-recorder")
        }
        fn parameters(&self) -> &ParamSet {
            &self.params
        }
        fn prepare(&mut self, _format: AudioFormat) {}
        fn reset(&mut self) {}
        fn process(&mut self, block: &mut AudioBlock<'_>) {
            block.apply_gain(self.params.value_f32(0));
        }
    }

    fn factory() -> Box<dyn AudioEffect> {
        Box::new(Recorder::new())
    }

    static REGISTRY: [EffectDef; 1] = [EffectDef::new(
        EffectKind::new("test-recorder"),
        "Recorder",
        "Test effect.",
        EffectCategory::Utility,
        factory,
    )];

    const CHANNELS: usize = 2;
    const TARGET_FILL: usize = 1_024;

    struct Harness {
        processor: RtProcessor,
        commands: Arc<ArrayQueue<ChainCommand>>,
        ring: Arc<AudioRing>,
        stats: Arc<StreamStats>,
        meters: Arc<SharedMeters>,
        _recycle: Arc<ArrayQueue<Box<dyn AudioEffect>>>,
    }

    impl Harness {
        fn new() -> Self {
            let format = AudioFormat::new(48_000, CHANNELS as u16);
            let commands = Arc::new(ArrayQueue::new(256));
            let recycle = recycle_queue();
            let ring = Arc::new(AudioRing::with_capacity(16_384));
            let meters = SharedMeters::new(CHANNELS);
            let stats = StreamStats::new();

            let processor = RtProcessor::new(
                format,
                Arc::clone(&commands),
                Arc::clone(&recycle),
                Arc::clone(&ring),
                Arc::clone(&meters),
                Arc::clone(&stats),
                TARGET_FILL,
                TARGET_FILL * 2,
            );

            Self {
                processor,
                commands,
                ring,
                stats,
                meters,
                _recycle: recycle,
            }
        }

        /// Primes the ring with `frames` of a known signal.
        fn prime(&self, frames: usize, value: f32) {
            let samples = vec![value; frames * CHANNELS];
            self.ring.write(&samples);
        }

        fn run(&mut self, frames: usize) -> (Vec<f32>, ProcessorReport) {
            let mut buffer = vec![0.0_f32; frames * CHANNELS];
            let report = self.processor.process(&mut buffer, frames);
            (buffer, report)
        }
    }

    #[test]
    fn an_empty_ring_produces_silence_and_is_reported_as_an_underrun() {
        let mut harness = Harness::new();
        let (buffer, report) = harness.run(480);

        assert_eq!(report.frames, 480);
        assert_eq!(report.sampled, 0);
        assert_eq!(report.underrun_samples, 480 * CHANNELS);
        assert!(!report.is_clean());
        assert!(buffer.iter().all(|s| *s == 0.0));
        assert_eq!(harness.stats.snapshot().underrun_samples, 960);
    }

    #[test]
    fn audio_round_trips_through_the_ring_and_the_chain() {
        let mut harness = Harness::new();
        harness.prime(480, 0.25);

        let (buffer, report) = harness.run(480);
        assert_eq!(report.sampled, 960);
        assert!(report.is_clean());
        assert!(buffer.iter().all(|s| (*s - 0.25).abs() < 1e-9));

        let snapshot = harness.stats.snapshot();
        assert_eq!(snapshot.processed_frames, 480);
        assert!(snapshot.is_healthy());
    }

    #[test]
    fn output_meters_reflect_the_processed_signal() {
        let mut harness = Harness::new();
        harness.prime(480, 0.5);
        harness.run(480);

        let snapshot = harness.meters.snapshot();
        assert_eq!(snapshot.channels(), CHANNELS);
        assert!((snapshot.peak_of(0) - 0.5).abs() < 1e-6);
        assert!((snapshot.rms_of(0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn queued_commands_change_the_chain() {
        let mut harness = Harness::new();
        assert_eq!(harness.processor.chain().len(), 0);

        harness
            .commands
            .push(ChainCommand::Insert {
                index: 0,
                effect: Box::new(Recorder::new()),
                enabled: true,
            })
            .ok()
            .unwrap();

        harness.prime(480, 0.25);
        let (_, report) = harness.run(480);

        assert_eq!(report.commands_applied, 1);
        assert_eq!(harness.processor.chain().len(), 1);
    }

    #[test]
    fn effect_parameters_take_effect_immediately() {
        let mut harness = Harness::new();
        let effect = Box::new(Recorder::new());
        let parameters = effect.parameters().clone();
        harness
            .commands
            .push(ChainCommand::Insert {
                index: 0,
                effect,
                enabled: true,
            })
            .ok()
            .unwrap();

        parameters.set_f32(0, 2.0);
        harness.prime(480, 0.25);
        let (buffer, _) = harness.run(480);

        assert!(buffer.iter().all(|s| (*s - 0.5).abs() < 1e-9));
    }

    #[test]
    fn removed_effects_are_recycled_rather_than_dropped() {
        let mut harness = Harness::new();
        harness
            .commands
            .push(ChainCommand::Insert {
                index: 0,
                effect: Box::new(Recorder::new()),
                enabled: true,
            })
            .ok()
            .unwrap();
        harness.prime(480, 0.0);
        harness.run(480);

        harness
            .commands
            .push(ChainCommand::Remove { index: 0 })
            .ok()
            .unwrap();
        harness.run(480);

        assert!(harness.processor.chain().is_empty());
        assert_eq!(
            harness.processor.recycle_queue().len(),
            1,
            "the retired effect should be waiting for the interface thread"
        );
        assert_eq!(harness.stats.snapshot().retired_drops, 0);
    }

    #[test]
    fn accumulated_latency_is_trimmed_back_to_the_target() {
        let mut harness = Harness::new();
        // Fill far beyond the high watermark.
        harness.prime(TARGET_FILL * 6 / CHANNELS, 0.1);
        assert!(harness.ring.len() > TARGET_FILL * 2);

        let (_, report) = harness.run(480);
        assert!(report.trimmed_samples > 0);
        assert_eq!(harness.stats.snapshot().trim_events, 1);
        // Back to roughly the target, having consumed one block as well.
        assert!(harness.ring.len() <= TARGET_FILL);
    }

    #[test]
    fn trimming_is_counted_but_is_not_a_glitch() {
        let mut harness = Harness::new();
        harness.prime(TARGET_FILL * 6 / CHANNELS, 0.1);
        harness.run(480);
        let snapshot = harness.stats.snapshot();
        assert!(snapshot.trimmed_samples > 0);
        assert!(snapshot.is_healthy());
        assert_eq!(snapshot.glitch_samples(), 0);
    }

    #[test]
    fn a_block_larger_than_the_buffer_is_clamped_rather_than_panicking() {
        let mut harness = Harness::new();
        let mut buffer = vec![0.0_f32; 64];
        let report = harness.processor.process(&mut buffer, 100_000);
        assert_eq!(report.frames, 32);
        assert_eq!(report.underrun_samples, 64);
    }

    #[test]
    fn a_zero_frame_block_is_harmless() {
        let mut harness = Harness::new();
        let mut buffer = vec![0.0_f32; 8];
        let report = harness.processor.process(&mut buffer, 0);
        assert_eq!(report.frames, 0);
        assert_eq!(report.sampled, 0);
        assert!(report.is_clean());
    }

    #[test]
    fn hostile_samples_do_not_escape_the_processor() {
        let mut harness = Harness::new();
        let mut samples = vec![0.0_f32; 480 * CHANNELS];
        samples[0] = f32::NAN;
        samples[1] = f32::INFINITY;
        harness.ring.write(&samples);

        let (buffer, _) = harness.run(480);
        assert!(
            buffer.iter().all(|s| s.is_finite()),
            "a non-finite sample reached the output"
        );
    }

    #[test]
    fn the_capture_source_fills_the_ring_and_meters_the_input() {
        let ring = Arc::new(AudioRing::with_capacity(4_096));
        let meters = SharedMeters::new(CHANNELS);
        let stats = StreamStats::new();
        let mut source = CaptureSource::new(
            Arc::clone(&ring),
            Arc::clone(&meters),
            Arc::clone(&stats),
            AudioFormat::new(48_000, CHANNELS as u16),
        );

        let written = source.capture(&[0.75; 480 * CHANNELS]);
        assert_eq!(written, 480 * CHANNELS);
        assert_eq!(ring.len(), 960);
        assert_eq!(stats.snapshot().capture_frames, 480);
        assert!((meters.snapshot().peak_of(0) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn the_capture_source_drops_rather_than_overwrites_when_full() {
        let ring = Arc::new(AudioRing::with_capacity(64));
        let meters = SharedMeters::new(CHANNELS);
        let stats = StreamStats::new();
        let mut source = CaptureSource::new(
            Arc::clone(&ring),
            Arc::clone(&meters),
            Arc::clone(&stats),
            AudioFormat::new(48_000, CHANNELS as u16),
        );

        source.capture(&[0.5; 64]);
        source.capture(&[0.5; 64]);

        // The first block survives intact; the second is dropped and counted.
        assert_eq!(ring.len(), 64);
        assert_eq!(stats.snapshot().overrun_samples, 64);
    }

    #[test]
    fn a_registry_effect_can_be_inserted_by_command() {
        // The chain commands carry `Box<dyn AudioEffect>` across threads, which
        // is the whole reason the trait is `Send`.
        let mut harness = Harness::new();
        let effect = REGISTRY[0].instantiate();
        harness
            .commands
            .push(ChainCommand::Insert {
                index: usize::MAX,
                effect,
                enabled: true,
            })
            .ok()
            .unwrap();
        harness.prime(480, 0.1);
        harness.run(480);
        assert_eq!(harness.processor.chain().len(), 1);
    }
}
