use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;

pub const MAX_BANDS: usize = 8;
pub const DEFAULT_BANDS: usize = 4;
pub const MAX_STAGES: usize = 2 * MAX_BANDS;
const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[derive(Clone)]
pub struct Crossover {
    filters: [[[Biquad; MAX_STAGES]; MAX_BANDS]; MAX_CHANNELS],
    lengths: [usize; MAX_BANDS],
    allpass: [[[Biquad; MAX_BANDS]; MAX_BANDS]; MAX_CHANNELS],
    count: usize,
}

impl Default for Crossover {
    fn default() -> Self {
        Self {
            filters: [[[Biquad::default(); MAX_STAGES]; MAX_BANDS]; MAX_CHANNELS],
            lengths: [0; MAX_BANDS],
            allpass: [[[Biquad::default(); MAX_BANDS]; MAX_BANDS]; MAX_CHANNELS],
            count: DEFAULT_BANDS,
        }
    }
}

impl Crossover {
    pub fn design(&mut self, count: usize, rate: f32, splits: &[f32]) {
        self.count = count.clamp(2, MAX_BANDS);
        self.lengths.fill(0);
        for band in 0..MAX_BANDS {
            if band >= self.count {
                continue;
            }
            let mut length = 0;
            for split in splits.iter().take(band) {
                let coefficients =
                    Coefficients::design(FilterKind::HighPass, rate, *split, BUTTERWORTH_Q, 0.0);
                self.assign(band, length, coefficients);
                length += 1;
                self.assign(band, length, coefficients);
                length += 1;
            }
            if band + 1 < self.count {
                let coefficients = Coefficients::design(
                    FilterKind::LowPass,
                    rate,
                    splits[band],
                    BUTTERWORTH_Q,
                    0.0,
                );
                self.assign(band, length, coefficients);
                length += 1;
                self.assign(band, length, coefficients);
                length += 1;
            }
            self.lengths[band] = length;
        }
        for split in 0..self.count.saturating_sub(1) {
            let Some(frequency) = splits.get(split).copied() else {
                break;
            };
            let coefficients =
                Coefficients::design(FilterKind::AllPass, rate, frequency, BUTTERWORTH_Q, 0.0);
            for band in 0..split {
                for channel in 0..MAX_CHANNELS {
                    self.allpass[channel][band][split].set_coefficients(coefficients);
                }
            }
        }
    }

    fn assign(&mut self, band: usize, stage: usize, coefficients: Coefficients) {
        if stage >= MAX_STAGES {
            return;
        }
        for channel in 0..MAX_CHANNELS {
            self.filters[channel][band][stage].set_coefficients(coefficients);
        }
    }

    pub fn reset(&mut self) {
        for channel in &mut self.filters {
            for band in channel {
                for stage in band {
                    stage.reset();
                }
            }
        }
        for channel in &mut self.allpass {
            for band in channel {
                for stage in band {
                    stage.reset();
                }
            }
        }
    }

    pub fn run(&mut self, channel: usize, band: usize, input: f32) -> f32 {
        if channel >= MAX_CHANNELS || band >= self.count {
            return input;
        }
        let mut value = input;
        for stage in 0..self.lengths[band] {
            value = self.filters[channel][band][stage].process(value);
        }
        for split in (band + 1)..self.count.saturating_sub(1) {
            value = self.allpass[channel][band][split].process(value);
        }
        value
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
