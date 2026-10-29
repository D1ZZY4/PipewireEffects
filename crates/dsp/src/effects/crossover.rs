use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;

pub const MAX_BANDS: usize = 4;
pub const MAX_STAGES: usize = 2 * MAX_BANDS;
const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[derive(Clone)]
pub struct Crossover {
    filters: [[[Biquad; MAX_STAGES]; MAX_BANDS]; MAX_CHANNELS],
    lengths: [usize; MAX_BANDS],
    count: usize,
}

impl Default for Crossover {
    fn default() -> Self {
        Self {
            filters: [[[Biquad::default(); MAX_STAGES]; MAX_BANDS]; MAX_CHANNELS],
            lengths: [0; MAX_BANDS],
            count: 2,
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
    }

    pub fn run(&mut self, channel: usize, band: usize, input: f32) -> f32 {
        if channel >= MAX_CHANNELS || band >= self.count {
            return input;
        }
        let mut value = input;
        for stage in 0..self.lengths[band] {
            value = self.filters[channel][band][stage].process(value);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn tone(frequency: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|index| (TAU * frequency * index as f32 / 48_000.0).sin())
            .collect()
    }

    #[test]
    fn the_bands_reconstruct_the_input_flat() {
        for frequency in [80.0f32, 300.0, 1_200.0, 6_000.0] {
            let mut crossover = Crossover::default();
            crossover.design(4, 48_000.0, &[120.0, 800.0, 5_000.0]);
            let input = tone(frequency, 16_384);
            let mut peak = 0.0f32;
            for (index, sample) in input.iter().enumerate() {
                let mut sum = 0.0f32;
                for band in 0..4 {
                    sum += crossover.run(0, band, *sample);
                }
                if index > 2_048 {
                    peak = peak.max(sum.abs());
                }
            }
            assert!(
                (peak - 1.0).abs() < 0.15,
                "{frequency} Hz reconstructed to {peak}"
            );
        }
    }

    #[test]
    fn a_well_separated_tone_dominates_its_band() {
        let mut crossover = Crossover::default();
        crossover.design(4, 48_000.0, &[120.0, 800.0, 5_000.0]);
        let input = tone(30.0, 16_384);
        let mut low = 0.0f32;
        let mut high = 0.0f32;
        for (index, sample) in input.iter().enumerate() {
            let low_value = crossover.run(0, 0, *sample);
            let mut top = 0.0f32;
            for band in 1..4 {
                top = top.max(crossover.run(0, band, *sample).abs());
            }
            if index > 4_096 {
                low = low.max(low_value.abs());
                high = high.max(top);
            }
        }
        assert!(low > 0.9, "low band only reached {low}");
        assert!(high * 5.0 < low, "high bands leaked {high} against {low}");
    }
}
