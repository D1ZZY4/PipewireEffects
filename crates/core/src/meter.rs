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

    pub fn reset(&mut self) {
        self.channels = [ChannelLevel::default(); MAX_CHANNELS];
    }

    pub fn process(&mut self, buffer: &AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        let rms_coefficient = self.rms_coefficient;
        let peak_decay = self.peak_decay;
        for index in 0..channels {
            let level = &mut self.channels[index];
            for sample in buffer.channel(index) {
                let value = sanitize(*sample);
                let magnitude = value.abs();
                level.peak = if magnitude >= level.peak {
                    magnitude
                } else {
                    level.peak * peak_decay
                };
                level.mean_square += (value * value - level.mean_square) * rms_coefficient;
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

    pub fn true_peak_db(&self) -> f32 {
        self.peak_db(0).max(self.peak_db(1))
    }

    pub fn loudness_db(&self) -> f32 {
        self.rms_db(0).max(self.rms_db(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frames: usize, amplitude: f32) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                let phase = std::f32::consts::TAU * 997.0 * index as f32 / 48_000.0;
                *sample = amplitude * phase.sin();
            }
        }
        buffer
    }

    #[test]
    fn peak_and_rms_track_a_sine() {
        let mut meter = LevelMeter::new();
        meter.set_rate(48_000);
        for _ in 0..200 {
            meter.process(&sine(1024, 0.5));
        }
        assert!((meter.peak(0) - 0.5).abs() < 0.02, "peak {}", meter.peak(0));
        assert!((meter.rms(0) - 0.3535).abs() < 0.02, "rms {}", meter.rms(0));
        assert!((meter.true_peak_db() - -6.02).abs() < 0.5);
    }

    #[test]
    fn silence_decays_to_the_floor() {
        let mut meter = LevelMeter::new();
        meter.set_rate(48_000);
        meter.process(&sine(4096, 1.0));
        let mut silent = AudioBuffer::new(2, 4096);
        silent.configure(2, 4096).unwrap();
        for _ in 0..2000 {
            meter.process(&silent);
        }
        assert!(meter.peak_db(0) < -100.0);
        assert!(meter.loudness_db() < -100.0);
    }
}
