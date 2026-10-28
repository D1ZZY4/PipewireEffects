use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::audio::AudioBlock;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeterSnapshot {
    pub peak: Vec<f32>,
    pub rms: Vec<f32>,
}

impl MeterSnapshot {
    #[must_use]
    pub fn channels(&self) -> usize {
        self.peak.len()
    }

    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.peak.iter().all(|value| *value <= f32::EPSILON)
    }

    #[must_use]
    pub fn peak_of(&self, channel: usize) -> f32 {
        self.peak.get(channel).copied().unwrap_or(0.0)
    }

    #[must_use]
    pub fn rms_of(&self, channel: usize) -> f32 {
        self.rms.get(channel).copied().unwrap_or(0.0)
    }

    #[must_use]
    pub fn loudest_peak(&self) -> f32 {
        self.peak.iter().copied().fold(0.0_f32, f32::max)
    }

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

#[derive(Debug)]
pub struct SharedMeters {
    peak: Box<[AtomicU32]>,
    rms: Box<[AtomicU32]>,
}

impl SharedMeters {
    #[must_use]
    pub fn new(channels: usize) -> Arc<Self> {
        let channels = channels.max(1);
        let peak = (0..channels).map(|_| AtomicU32::new(0)).collect();
        let rms = (0..channels).map(|_| AtomicU32::new(0)).collect();
        Arc::new(Self { peak, rms })
    }

    #[must_use]
    pub fn channels(&self) -> usize {
        self.peak.len()
    }

    pub fn store(&self, peak: &[f32], rms: &[f32]) {
        for (cell, value) in self.peak.iter().zip(peak) {
            store_channel(cell, *value);
        }
        for (cell, value) in self.rms.iter().zip(rms) {
            store_channel(cell, *value);
        }
    }

    pub fn reset(&self) {
        for cell in self.peak.iter() {
            store_channel(cell, 0.0);
        }
        for cell in self.rms.iter() {
            store_channel(cell, 0.0);
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> MeterSnapshot {
        MeterSnapshot {
            peak: self.peak.iter().map(load_channel).collect(),
            rms: self.rms.iter().map(load_channel).collect(),
        }
    }
}

#[derive(Debug)]
pub struct BlockMeter {
    peak: Vec<f32>,
    sum_squares: Vec<f32>,
    rms: Vec<f32>,
    frames: f32,
}

impl BlockMeter {
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

    #[must_use]
    pub fn channels(&self) -> usize {
        self.peak.len()
    }

    pub fn reset(&mut self) {
        self.peak.fill(0.0);
        self.sum_squares.fill(0.0);
        self.frames = 0.0;
    }

    pub fn accumulate(&mut self, samples: &[f32], channels: usize) {
        if channels == 0 || channels != self.peak.len() {
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

    pub fn accumulate_block(&mut self, block: &AudioBlock<'_>) {
        self.accumulate(block.active_samples(), block.channels());
    }

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
mod tests;
