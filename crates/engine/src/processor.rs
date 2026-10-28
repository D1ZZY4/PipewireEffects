use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use pipewire_effects_core::{
    AudioBlock, AudioEffect, AudioFormat, AudioRing, BlockMeter, ChainCommand, EffectChain,
    SharedMeters,
};

use crate::stats::StreamStats;

const MAX_COMMANDS_PER_BLOCK: usize = 16;

const RECYCLE_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessorReport {
    pub frames: usize,
    pub sampled: usize,
    pub underrun_samples: usize,
    pub trimmed_samples: usize,
    pub commands_applied: usize,
}

impl ProcessorReport {
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.underrun_samples == 0 && self.trimmed_samples == 0
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.underrun_samples == 0
    }
}

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

    #[must_use]
    pub fn chain(&self) -> &EffectChain {
        &self.chain
    }

    #[must_use]
    pub fn recycle_queue(&self) -> &Arc<ArrayQueue<Box<dyn AudioEffect>>> {
        &self.recycle
    }

    pub fn set_format(&mut self, format: AudioFormat) {
        self.format = format;
        self.channels = format.channels_usize().max(1);
        self.chain.set_format(format);
        self.meter = BlockMeter::new(self.channels);
    }

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
                    stats.add_retired_drop();
                }
            });
            applied += 1;
        }
        applied
    }

    pub fn process(&mut self, buffer: &mut [f32], frames: usize) -> ProcessorReport {
        let channels = self.channels;
        let frames = frames.min(buffer.len() / channels);
        let needed = frames * channels;

        let mut report = ProcessorReport {
            frames,
            commands_applied: self.drain_commands(),
            ..ProcessorReport::default()
        };

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
            buffer[read.moved..needed].fill(0.0);
            self.stats.add_underrun_samples(read.shortfall as u64);
            report.underrun_samples = read.shortfall;
        }

        if frames > 0 {
            let mut block = AudioBlock::with_frames(&mut buffer[..needed], self.format, frames);
            self.chain.process(&mut block);
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

pub struct CaptureSource {
    ring: Arc<AudioRing>,
    input_meters: Arc<SharedMeters>,
    stats: Arc<StreamStats>,
    meter: BlockMeter,
    channels: usize,
}

impl CaptureSource {
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

#[must_use]
pub fn recycle_queue() -> Arc<ArrayQueue<Box<dyn AudioEffect>>> {
    Arc::new(ArrayQueue::new(RECYCLE_CAPACITY))
}

#[cfg(test)]
mod tests;
