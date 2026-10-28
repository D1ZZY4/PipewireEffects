use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("frequency", "Crossover", ParamUnit::Hertz)
            .range(20.0, 500.0, 150.0)
            .logarithmic()
            .group("Band")
            .build(),
        ParamSpec::float("amount", "Harmonics", ParamUnit::Percent)
            .range(0.0, 100.0, 30.0)
            .step(1.0)
            .group("Band")
            .build(),
        ParamSpec::float("gain", "Output gain", ParamUnit::Decibel)
            .range(-12.0, 12.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    ]
}

pub struct BassEnhancer {
    frequency: Knob,
    amount: Knob,
    gain: Knob,
    low_pass: [Biquad; MAX_CHANNELS],
    high_pass: [Biquad; MAX_CHANNELS],
    signature: [f32; 2],
    rate: f32,
    dirty: bool,
}

impl BassEnhancer {
    pub fn new(params: ParamSet) -> Self {
        Self {
            frequency: Knob::new(&params, "frequency", 150.0),
            amount: Knob::new(&params, "amount", 30.0),
            gain: Knob::new(&params, "gain", 0.0),
            low_pass: [Biquad::default(); MAX_CHANNELS],
            high_pass: [Biquad::default(); MAX_CHANNELS],
            signature: [f32::NAN; 2],
            rate: 48_000.0,
            dirty: true,
        }
    }

    fn refresh(&mut self) {
        let frequency = self.frequency.value();
        let low = Coefficients::design(
            FilterKind::LowPass,
            self.rate,
            frequency,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
        );
        let high = Coefficients::design(
            FilterKind::HighPass,
            self.rate,
            frequency,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
        );
        for channel in 0..MAX_CHANNELS {
            self.low_pass[channel].set_coefficients(low);
            self.high_pass[channel].set_coefficients(high);
        }
    }
}

impl Effect for BassEnhancer {
    fn id(&self) -> &'static str {
        "bass_enhancer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for biquad in &mut self.low_pass {
            biquad.reset();
        }
        for biquad in &mut self.high_pass {
            biquad.reset();
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let signature = [self.frequency.value(), self.amount.value()];
        if self.dirty || signature != self.signature {
            self.signature = signature;
            self.dirty = false;
            self.refresh();
        }
        let amount = (signature[1] / 100.0).clamp(0.0, 1.0) * 1.5;
        let output_gain = math::db_to_gain(self.gain.value());
        let channels = buffer.channels().min(MAX_CHANNELS);

        for channel in 0..channels {
            let low_pass = &mut self.low_pass[channel];
            let high_pass = &mut self.high_pass[channel];
            for sample in buffer.channel_mut(channel) {
                let input = *sample * output_gain;
                let low = low_pass.process(input);
                let harmonics = high_pass.process(low.abs());
                *sample = math::sanitize(input + harmonics * amount);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample = 0.5 * (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    #[test]
    fn a_bass_tone_gains_harmonic_content() {
        let params = ParamSet::new(specs());
        params.set_f32("amount", 100.0).unwrap();
        let mut effect = BassEnhancer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(60.0, 4096);
        for _ in 0..64 {
            buffer = tone(60.0, 4096);
            effect.process(&mut buffer);
        }
        assert!(buffer.peak() > 0.5, "peak {}", buffer.peak());
    }

    #[test]
    fn with_zero_amount_the_signal_is_unchanged() {
        let params = ParamSet::new(specs());
        params.set_f32("amount", 0.0).unwrap();
        let mut effect = BassEnhancer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(60.0, 2048);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.5).abs() < 0.01, "peak {}", buffer.peak());
    }

    #[test]
    fn output_stays_finite() {
        let params = ParamSet::new(specs());
        params.set_f32("amount", 100.0).unwrap();
        params.set_f32("gain", 12.0).unwrap();
        let mut effect = BassEnhancer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..64 {
            let mut buffer = tone(40.0, 4096);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 8.0, "{sample}");
                }
            }
        }
    }
}
