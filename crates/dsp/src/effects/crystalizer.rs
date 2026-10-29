use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const BANDS: usize = 4;
const CROSSOVERS: [f32; 3] = [200.0, 2_000.0, 8_000.0];
const BAND_IDS: [&str; BANDS] = ["band1", "band2", "band3", "band4"];
const BAND_LABELS: [&str; BANDS] = [
    mark("Below 200 Hz"),
    mark("200 Hz to 2 kHz"),
    mark("2 kHz to 8 kHz"),
    mark("Above 8 kHz"),
];

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = vec![
        ParamSpec::float("amount", "Amount", ParamUnit::Percent)
            .range(0.0, 100.0, 30.0)
            .step(1.0)
            .group("Effect")
            .build(),
        ParamSpec::float("gain", "Output gain", ParamUnit::Decibel)
            .range(-12.0, 12.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    ];
    for (id, label) in BAND_IDS.into_iter().zip(BAND_LABELS) {
        specs.push(ParamSpec::boolean(id, label, true).group("Bands"));
    }
    specs
}

pub struct Crystalizer {
    amount: Knob,
    gain: Knob,
    bands: [Knob; BANDS],
    filters: [[Biquad; 3]; MAX_CHANNELS],
    fast: [[f32; BANDS]; MAX_CHANNELS],
    slow: [[f32; BANDS]; MAX_CHANNELS],
    fast_coefficient: f32,
    slow_coefficient: f32,
    rate: u32,
}

impl Crystalizer {
    pub fn new(params: ParamSet) -> Self {
        Self {
            amount: Knob::new(&params, "amount", 30.0),
            gain: Knob::new(&params, "gain", 0.0),
            bands: std::array::from_fn(|band| Knob::new(&params, BAND_IDS[band], 1.0)),
            filters: [[Biquad::default(); 3]; MAX_CHANNELS],
            fast: [[0.0; BANDS]; MAX_CHANNELS],
            slow: [[0.0; BANDS]; MAX_CHANNELS],
            fast_coefficient: 0.0,
            slow_coefficient: 0.0,
            rate: 48_000,
        }
    }

    fn refresh(&mut self) {
        self.fast_coefficient = math::one_pole_alpha(5.0, self.rate);
        self.slow_coefficient = math::one_pole_alpha(250.0, self.rate);
        for channel in 0..MAX_CHANNELS {
            for (index, crossover) in CROSSOVERS.iter().enumerate() {
                let coefficients = Coefficients::design(
                    FilterKind::LowPass,
                    self.rate as f32,
                    *crossover,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                );
                self.filters[channel][index].set_coefficients(coefficients);
            }
        }
    }
}

impl Effect for Crystalizer {
    fn id(&self) -> &'static str {
        "crystalizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.refresh();
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.filters {
            for biquad in channel {
                biquad.reset();
            }
        }
        self.fast = [[0.0; BANDS]; MAX_CHANNELS];
        self.slow = [[0.0; BANDS]; MAX_CHANNELS];
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let amount = (self.amount.value() / 100.0).clamp(0.0, 1.0) * 3.0;
        let output_gain = math::db_to_gain(self.gain.value());
        let enabled: [bool; BANDS] = std::array::from_fn(|band| self.bands[band].flag());
        let channels = buffer.channels().min(MAX_CHANNELS);
        let fast_coefficient = self.fast_coefficient;
        let slow_coefficient = self.slow_coefficient;

        for channel in 0..channels {
            let low_pass = &mut self.filters[channel];
            let fast = &mut self.fast[channel];
            let slow = &mut self.slow[channel];
            for sample in buffer.channel_mut(channel) {
                let input = *sample;
                let band0 = low_pass[0].process(input);
                let rest0 = input - band0;
                let band1 = low_pass[1].process(rest0);
                let rest1 = rest0 - band1;
                let band2 = low_pass[2].process(rest1);
                let band3 = rest1 - band2;
                let split = [band0, band1, band2, band3];
                let mut sum = 0.0f32;
                for band in 0..BANDS {
                    if enabled[band] {
                        let rectified = split[band].abs();
                        fast[band] += (rectified - fast[band]) * fast_coefficient;
                        slow[band] += (rectified - slow[band]) * slow_coefficient;
                        let transient = (fast[band] - slow[band]) * amount;
                        sum += split[band] * (1.0 + transient.clamp(-0.9, 1.5));
                    } else {
                        sum += split[band];
                    }
                }
                *sample = math::sanitize(sum * output_gain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample =
                    amplitude * (std::f32::consts::TAU * 3_000.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    fn impulse(frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        buffer.channel_mut(0)[0] = 1.0;
        buffer.channel_mut(1)[0] = 1.0;
        buffer
    }

    #[test]
    fn transients_are_emphasised() {
        let params = ParamSet::new(specs());
        params.set_f32("amount", 100.0).unwrap();
        let mut effect = Crystalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = impulse(64);
        effect.process(&mut buffer);
        assert!(buffer.peak() > 1.0, "peak {}", buffer.peak());
    }

    #[test]
    fn a_steady_tone_is_barely_changed() {
        let params = ParamSet::new(specs());
        params.set_f32("amount", 100.0).unwrap();
        let mut effect = Crystalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..64 {
            let mut buffer = tone(0.5, 1024);
            effect.process(&mut buffer);
        }
        let mut buffer = tone(0.5, 1024);
        effect.process(&mut buffer);
        assert!(buffer.peak() < 0.75, "peak {}", buffer.peak());
    }

    #[test]
    fn disabled_bands_are_returned_unprocessed() {
        let params = ParamSet::new(specs());
        params.set_f32("amount", 100.0).unwrap();
        for id in BAND_IDS {
            params.set_bool(id, false).unwrap();
        }
        let mut effect = Crystalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = impulse(64);
        let reference = buffer.channel(0).to_vec();
        effect.process(&mut buffer);
        for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
            assert!((processed - original).abs() < 1.0e-5);
        }
    }

    #[test]
    fn output_stays_finite_under_abuse() {
        let params = ParamSet::new(specs());
        params.set_f32("amount", 100.0).unwrap();
        params.set_f32("gain", 12.0).unwrap();
        let mut effect = Crystalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..200 {
            let mut buffer = impulse(1024);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 32.0, "{sample}");
                }
            }
        }
    }
}
