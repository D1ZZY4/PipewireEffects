use crate::audio::AudioBuffer;
use crate::math::{gain_to_db, one_pole_alpha, sanitize};

pub const MAX_CHANNELS: usize = 2;

#[derive(Debug, Clone, Copy, Default)]
struct ChannelLevel {
    peak: f32,
    mean_square: f32,
}

#[derive(Debug, Clone)]
pub struct LevelMeter {
    channels: [ChannelLevel; MAX_CHANNELS],
    rms_coefficient: f32,
    peak_decay: f32,
}

impl Default for LevelMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelMeter {
    pub fn new() -> Self {
        let mut meter = Self {
            channels: [ChannelLevel::default(); MAX_CHANNELS],
            rms_coefficient: 0.0,
            peak_decay: 0.0,
        };
        meter.set_rate(48_000);
        meter
    }

    pub fn set_rate(&mut self, rate: u32) {
        let rate = rate.max(1);
        self.rms_coefficient = one_pole_alpha(300.0, rate);
        self.peak_decay = 1.0 - one_pole_alpha(1200.0, rate);
    }

    pub fn process(&mut self, buffer: &AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        let rms_coefficient = self.rms_coefficient;
        let peak_decay = self.peak_decay;
        for index in 0..channels {
            for sample in buffer.channel(index) {
                Self::advance(
                    &mut self.channels[index],
                    sanitize(*sample),
                    rms_coefficient,
                    peak_decay,
                );
            }
        }
    }

    fn advance(level: &mut ChannelLevel, value: f32, rms_coefficient: f32, peak_decay: f32) {
        let magnitude = value.abs();
        level.peak = if magnitude >= level.peak {
            magnitude
        } else {
            level.peak * peak_decay
        };
        level.mean_square += (value * value - level.mean_square) * rms_coefficient;
    }

    pub fn process_interleaved(&mut self, samples: &[f32], channels: usize) {
        let channels = channels.min(MAX_CHANNELS);
        if channels == 0 {
            return;
        }
        let rms_coefficient = self.rms_coefficient;
        let peak_decay = self.peak_decay;
        for (index, sample) in samples.iter().enumerate() {
            let channel = index % channels;
            Self::advance(
                &mut self.channels[channel],
                sanitize(*sample),
                rms_coefficient,
                peak_decay,
            );
        }
    }

    pub fn peak(&self, channel: usize) -> f32 {
        self.channels.get(channel).map_or(0.0, |level| level.peak)
    }

    pub fn rms(&self, channel: usize) -> f32 {
        self.channels
            .get(channel)
            .map_or(0.0, |level| level.mean_square)
            .max(0.0)
            .sqrt()
    }

    pub fn peak_db(&self, channel: usize) -> f32 {
        gain_to_db(self.peak(channel))
    }

    pub fn rms_db(&self, channel: usize) -> f32 {
        gain_to_db(self.rms(channel))
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
