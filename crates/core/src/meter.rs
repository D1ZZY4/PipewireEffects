//! Level metering.
//!
//! Meters are written by the audio thread and read by the interface, so they
//! are shared through atomics: the audio thread never waits, and a meter that
//! is one or two frames stale does not matter visually.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::audio::AudioBlock;

/// A per-channel level reading, as linear amplitudes in the `0.0..=n` range.
///
/// Values above `1.0` are possible and mean the signal is clipping; the
/// interface is responsible for showing that.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeterSnapshot {
    /// The largest absolute sample seen in the last block, per channel.
    pub peak: Vec<f32>,
    /// The root-mean-square level of the last block, per channel.
    pub rms: Vec<f32>,
}

impl MeterSnapshot {
    /// The number of channels described.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.peak.len()
    }

    /// Whether every channel is silent.
    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.peak.iter().all(|value| *value <= f32::EPSILON)
    }

    /// The peak of one channel, or zero when the channel does not exist.
    #[must_use]
    pub fn peak_of(&self, channel: usize) -> f32 {
        self.peak.get(channel).copied().unwrap_or(0.0)
    }

    /// The RMS level of one channel, or zero when the channel does not exist.
    #[must_use]
    pub fn rms_of(&self, channel: usize) -> f32 {
        self.rms.get(channel).copied().unwrap_or(0.0)
    }

    /// The loudest peak across every channel.
    #[must_use]
    pub fn loudest_peak(&self) -> f32 {
        self.peak.iter().copied().fold(0.0_f32, f32::max)
    }

    /// Whether any channel is at or above full scale.
    #[must_use]
    pub fn is_clipping(&self) -> bool {
        self.loudest_peak() >= 1.0
    }
}

fn store_channel(cell: &AtomicU32, value: f32) {
    cell.store(value.to_bits(), Ordering::Relaxed);
}

fn load_channel(cell: &AtomicU32) -> f32 {
    f32::from_bits(cell.load(Ordering::Relaxed))
}

/// The atomic store the audio thread writes meters into.
#[derive(Debug)]
pub struct SharedMeters {
    peak: Box<[AtomicU32]>,
    rms: Box<[AtomicU32]>,
}

impl SharedMeters {
    /// Creates a meter bank for `channels` channels, in a reference-counted
    /// handle that can be cloned onto both threads.
    #[must_use]
    pub fn new(channels: usize) -> Arc<Self> {
        let channels = channels.max(1);
        let peak = (0..channels).map(|_| AtomicU32::new(0)).collect();
        let rms = (0..channels).map(|_| AtomicU32::new(0)).collect();
        Arc::new(Self { peak, rms })
    }

    /// The number of channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.peak.len()
    }

    /// Publishes a reading. Called from the audio thread.
    ///
    /// Extra values are ignored and missing values are left untouched, so a
    /// channel-count change cannot panic here.
    pub fn store(&self, peak: &[f32], rms: &[f32]) {
        for (cell, value) in self.peak.iter().zip(peak) {
            store_channel(cell, *value);
        }
        for (cell, value) in self.rms.iter().zip(rms) {
            store_channel(cell, *value);
        }
    }

    /// Zeroes every reading.
    pub fn reset(&self) {
        for cell in self.peak.iter() {
            store_channel(cell, 0.0);
        }
        for cell in self.rms.iter() {
            store_channel(cell, 0.0);
        }
    }

    /// Reads the current reading. Called from the interface thread.
    #[must_use]
    pub fn snapshot(&self) -> MeterSnapshot {
        MeterSnapshot {
            peak: self.peak.iter().map(load_channel).collect(),
            rms: self.rms.iter().map(load_channel).collect(),
        }
    }
}

/// Per-block accumulation of peak and RMS, with no allocation in steady state.
///
/// The audio thread accumulates once per block and publishes once per block;
/// the scratch vectors are sized once, in [`BlockMeter::new`].
#[derive(Debug)]
pub struct BlockMeter {
    peak: Vec<f32>,
    sum_squares: Vec<f32>,
    rms: Vec<f32>,
    frames: f32,
}

impl BlockMeter {
    /// Creates an accumulator for `channels` channels.
    #[must_use]
    pub fn new(channels: usize) -> Self {
        let channels = channels.max(1);
        Self {
            peak: vec![0.0; channels],
            sum_squares: vec![0.0; channels],
            rms: vec![0.0; channels],
            frames: 0.0,
        }
    }

    /// The number of channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.peak.len()
    }

    /// Clears the accumulators.
    pub fn reset(&mut self) {
        self.peak.fill(0.0);
        self.sum_squares.fill(0.0);
        self.frames = 0.0;
    }

    /// Folds one buffer of interleaved samples into the accumulators.
    ///
    /// Takes the samples unsized by direction so that it can be used on the
    /// capture path, where the audio arrives as a read-only slice and building
    /// an [`AudioBlock`] (which needs `&mut`) would be wasteful.
    pub fn accumulate(&mut self, samples: &[f32], channels: usize) {
        if channels == 0 || channels != self.peak.len() {
            // The stream format changed underneath us. Resizing on the audio
            // thread is not acceptable, so skip this block; the engine rebuilds
            // the meter when it re-negotiates the format.
            return;
        }

        let frames = samples.len() / channels;
        #[allow(clippy::cast_precision_loss)]
        {
            self.frames += frames as f32;
        }

        for frame in samples[..frames * channels].chunks_exact(channels) {
            for (channel, sample) in frame.iter().enumerate() {
                let magnitude = sample.abs();
                if magnitude > self.peak[channel] {
                    self.peak[channel] = magnitude;
                }
                self.sum_squares[channel] += sample * sample;
            }
        }
    }

    /// Folds an [`AudioBlock`] into the accumulators.
    pub fn accumulate_block(&mut self, block: &AudioBlock<'_>) {
        self.accumulate(block.active_samples(), block.channels());
    }

    /// Computes RMS, publishes to `meters`, and clears the accumulators.
    pub fn publish(&mut self, meters: &SharedMeters) {
        if self.frames > 0.0 {
            for (rms, sum) in self.rms.iter_mut().zip(&self.sum_squares) {
                *rms = (sum / self.frames).sqrt();
            }
        } else {
            self.rms.fill(0.0);
        }
        meters.store(&self.peak, &self.rms);
        self.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioBlock, AudioFormat};

    fn stereo_block(samples: &mut [f32]) -> AudioBlock<'_> {
        AudioBlock::new(samples, AudioFormat::default_stereo())
    }

    #[test]
    fn accumulation_reports_per_channel_peak_and_rms() {
        let mut samples = [0.5_f32, -0.25, 1.0, 0.0];
        let block = stereo_block(&mut samples);
        let mut meter = BlockMeter::new(2);
        meter.accumulate_block(&block);
        let shared = SharedMeters::new(2);
        meter.publish(&shared);

        let snapshot = shared.snapshot();
        assert_eq!(snapshot.channels(), 2);
        assert!((snapshot.peak_of(0) - 1.0).abs() < f32::EPSILON);
        assert!((snapshot.peak_of(1) - 0.25).abs() < f32::EPSILON);
        assert!((snapshot.rms_of(0) - 0.625_f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn publishing_clears_the_accumulators() {
        let mut samples = [1.0_f32, 1.0];
        let block = stereo_block(&mut samples);
        let mut meter = BlockMeter::new(2);
        meter.accumulate_block(&block);
        let shared = SharedMeters::new(2);
        meter.publish(&shared);
        meter.publish(&shared);
        assert!(!shared.snapshot().is_clipping());
        assert!(shared.snapshot().is_silent());
    }

    #[test]
    fn clipping_is_detected_at_full_scale() {
        let mut samples = [1.0_f32, 0.0];
        let block = stereo_block(&mut samples);
        let mut meter = BlockMeter::new(2);
        meter.accumulate_block(&block);
        let shared = SharedMeters::new(2);
        meter.publish(&shared);
        assert!(shared.snapshot().is_clipping());
    }

    #[test]
    fn mismatch_between_block_and_meter_channels_is_survivable() {
        let mut samples = [0.0_f32; 6];
        let block = AudioBlock::new(&mut samples, AudioFormat::new(48_000, 6));
        let mut meter = BlockMeter::new(2);
        meter.accumulate_block(&block);
        let shared = SharedMeters::new(2);
        meter.publish(&shared);
        assert!(shared.snapshot().is_silent());
    }

    #[test]
    fn storing_fewer_values_than_channels_leaves_the_rest_alone() {
        let meters = SharedMeters::new(2);
        meters.store(&[0.9], &[0.4]);
        let snapshot = meters.snapshot();
        assert!((snapshot.peak_of(0) - 0.9).abs() < f32::EPSILON);
        assert!(snapshot.peak_of(1).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_zeroes_every_channel() {
        let meters = SharedMeters::new(4);
        meters.store(&[1.0; 4], &[1.0; 4]);
        meters.reset();
        assert!(meters.snapshot().is_silent());
    }
}
