use pwe_core::i18n::mark;

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;

pub const MODES: [&str; 4] = [
    mark("Off"),
    mark("12 dB/oct"),
    mark("24 dB/oct"),
    mark("36 dB/oct"),
];

const SECTIONS: usize = 3;

pub struct SidechainFilter {
    rate: u32,
    signature: [u32; 4],
    active: bool,
    high_pass: [[Biquad; SECTIONS]; MAX_CHANNELS],
    low_pass: [[Biquad; SECTIONS]; MAX_CHANNELS],
}

impl SidechainFilter {
    pub fn new(rate: u32) -> Self {
        let mut filter = Self {
            rate: rate.max(1),
            signature: [0; 4],
            active: false,
            high_pass: std::array::from_fn(|_| std::array::from_fn(|_| passthrough(rate))),
            low_pass: std::array::from_fn(|_| std::array::from_fn(|_| passthrough(rate))),
        };
        filter.update(0, 10.0, 0, 20_000.0);
        filter
    }

    pub fn prepare(&mut self, rate: u32) {
        self.rate = rate.max(1);
        self.signature = [0; 4];
        self.high_pass = std::array::from_fn(|_| std::array::from_fn(|_| passthrough(self.rate)));
        self.low_pass = std::array::from_fn(|_| std::array::from_fn(|_| passthrough(self.rate)));
    }

    pub fn bypassed(&self) -> bool {
        !self.active
    }

    pub fn update(&mut self, high_mode: usize, high_hz: f32, low_mode: usize, low_hz: f32) {
        let wanted = [
            high_mode as u32,
            high_hz.to_bits(),
            low_mode as u32,
            low_hz.to_bits(),
        ];
        if wanted == self.signature {
            return;
        }
        self.signature = wanted;
        self.active = high_mode > 0 || low_mode > 0;
        let high_sections = high_mode.min(SECTIONS);
        let low_sections = low_mode.min(SECTIONS);
        for channel in 0..MAX_CHANNELS {
            for section in 0..SECTIONS {
                self.high_pass[channel][section] = if section < high_sections {
                    Biquad::new(Coefficients::design(
                        FilterKind::HighPass,
                        self.rate as f32,
                        high_hz,
                        0.707,
                        0.0,
                    ))
                } else {
                    passthrough(self.rate)
                };
                self.low_pass[channel][section] = if section < low_sections {
                    Biquad::new(Coefficients::design(
                        FilterKind::LowPass,
                        self.rate as f32,
                        low_hz,
                        0.707,
                        0.0,
                    ))
                } else {
                    passthrough(self.rate)
                };
            }
        }
    }

    pub fn reset(&mut self) {
        for channel in &mut self.high_pass {
            for section in channel {
                section.reset();
            }
        }
        for channel in &mut self.low_pass {
            for section in channel {
                section.reset();
            }
        }
    }

    pub fn process(&mut self, channel: usize, sample: f32) -> f32 {
        let channel = channel.min(MAX_CHANNELS - 1);
        let mut value = sample;
        for section in &mut self.high_pass[channel] {
            value = section.process(value);
        }
        for section in &mut self.low_pass[channel] {
            value = section.process(value);
        }
        value
    }
}

fn passthrough(rate: u32) -> Biquad {
    Biquad::new(Coefficients::design(
        FilterKind::LowPass,
        rate as f32,
        20_000.0,
        0.707,
        0.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwe_core::AudioBuffer;

    fn tone(hz: f32, amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(1, frames);
        buffer.configure(1, frames).unwrap();
        let plan = buffer.channel_mut(0);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * hz * index as f32 / 48_000.0).sin();
        }
        buffer
    }

    fn measure(mode: usize, frequency: f32, hz: f32) -> f32 {
        let mut filter = SidechainFilter::new(48_000);
        filter.update(mode, frequency, 0, 20_000.0);
        let buffer = tone(hz, 0.5, 8_192);
        let mut peak = 0.0f32;
        for index in 0..8_192 {
            let value = filter.process(0, buffer.channel(0)[index]);
            if index >= 4_096 {
                peak = peak.max(value.abs());
            }
        }
        peak
    }

    #[test]
    fn the_high_pass_removes_the_low_tone_below_its_cutoff() {
        let open = measure(0, 10.0, 60.0);
        let filtered = measure(1, 400.0, 60.0);
        assert!(open > 0.4, "{open}");
        assert!(filtered < open * 0.2, "{filtered} against {open}");
    }

    #[test]
    fn a_steeper_high_pass_cut_more() {
        let one = measure(1, 400.0, 60.0);
        let three = measure(3, 400.0, 60.0);
        assert!(three < one, "{three} !< {one}");
    }

    #[test]
    fn the_high_pass_leaves_the_passband_alone() {
        let open = measure(0, 10.0, 4_000.0);
        let filtered = measure(3, 400.0, 4_000.0);
        assert!((open - filtered).abs() < 0.05, "{open} against {filtered}");
    }
}
