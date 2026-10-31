use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const RESONANCE: f32 = std::f32::consts::FRAC_1_SQRT_2;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("frequency", "Crossover", ParamUnit::Hertz)
            .range(20.0, 500.0, 150.0)
            .logarithmic()
            .group("Band")
            .build(),
        ParamSpec::float("amount", "Amount", ParamUnit::Percent)
            .range(0.0, 100.0, 30.0)
            .step(1.0)
            .group("Band")
            .build(),
        ParamSpec::float("harmonics", "Harmonics", ParamUnit::None)
            .range(0.1, 10.0, 8.5)
            .step(0.1)
            .group("Band")
            .build(),
        ParamSpec::float("blend", "Blend", ParamUnit::None)
            .range(-10.0, 10.0, 0.0)
            .step(0.1)
            .group("Band")
            .build(),
        ParamSpec::float("floor", "Floor", ParamUnit::Hertz)
            .range(10.0, 120.0, 20.0)
            .logarithmic()
            .group("Band")
            .build(),
        ParamSpec::boolean("floor_active", "Floor active", false).group("Band"),
        ParamSpec::boolean("listen", "Listen", false).group("Output"),
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
    harmonics: Knob,
    blend: Knob,
    floor: Knob,
    floor_active: Knob,
    listen: Knob,
    gain: Knob,
    low_a: [Biquad; MAX_CHANNELS],
    low_b: [Biquad; MAX_CHANNELS],
    low_c: [Biquad; MAX_CHANNELS],
    low_d: [Biquad; MAX_CHANNELS],
    floor_a: [Biquad; MAX_CHANNELS],
    floor_b: [Biquad; MAX_CHANNELS],
    rate: f32,
    signature: [f32; 2],
    dirty: bool,
}

impl BassEnhancer {
    pub fn new(params: ParamSet) -> Self {
        Self {
            frequency: Knob::new(&params, "frequency", 150.0),
            amount: Knob::new(&params, "amount", 30.0),
            harmonics: Knob::new(&params, "harmonics", 8.5),
            blend: Knob::new(&params, "blend", 0.0),
            floor: Knob::new(&params, "floor", 20.0),
            floor_active: Knob::new(&params, "floor_active", 0.0),
            listen: Knob::new(&params, "listen", 0.0),
            gain: Knob::new(&params, "gain", 0.0),
            low_a: [Biquad::default(); MAX_CHANNELS],
            low_b: [Biquad::default(); MAX_CHANNELS],
            low_c: [Biquad::default(); MAX_CHANNELS],
            low_d: [Biquad::default(); MAX_CHANNELS],
            floor_a: [Biquad::default(); MAX_CHANNELS],
            floor_b: [Biquad::default(); MAX_CHANNELS],
            rate: 48_000.0,
            signature: [f32::NAN; 2],
            dirty: true,
        }
    }

    fn refresh(&mut self) {
        let frequency = self.frequency.value().clamp(20.0, 500.0);
        let floor = self.floor.value().clamp(10.0, 120.0);
        let low = Coefficients::design(FilterKind::LowPass, self.rate, frequency, RESONANCE, 0.0);
        let high = Coefficients::design(FilterKind::HighPass, self.rate, floor, RESONANCE, 0.0);
        for channel in 0..MAX_CHANNELS {
            self.low_a[channel].set_coefficients(low);
            self.low_b[channel].set_coefficients(low);
            self.low_c[channel].set_coefficients(low);
            self.low_d[channel].set_coefficients(low);
            self.floor_a[channel].set_coefficients(high);
            self.floor_b[channel].set_coefficients(high);
        }
    }

    fn shape(driven: f32, blend: f32) -> f32 {
        let soft = driven / (1.0 + driven.abs());
        let even = driven.abs() / (1.0 + driven.abs()) - 0.25;
        soft * (1.0 - blend) + even * blend
    }
}

impl Effect for BassEnhancer {
    fn id(&self) -> &'static str {
        "bass_enhancer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(1) as f32;
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for biquad in &mut self.low_a {
            biquad.reset();
        }
        for biquad in &mut self.low_b {
            biquad.reset();
        }
        for biquad in &mut self.low_c {
            biquad.reset();
        }
        for biquad in &mut self.low_d {
            biquad.reset();
        }
        for biquad in &mut self.floor_a {
            biquad.reset();
        }
        for biquad in &mut self.floor_b {
            biquad.reset();
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let signature = [self.frequency.value(), self.floor.value()];
        if self.dirty || signature != self.signature {
            self.signature = signature;
            self.dirty = false;
            self.refresh();
        }
        let amount = (self.amount.value() / 100.0).clamp(0.0, 1.0) * 1.5;
        let drive = self.harmonics.value().clamp(0.1, 10.0);
        let blend = ((self.blend.value() + 10.0) / 20.0).clamp(0.0, 1.0);
        let floor_active = self.floor_active.flag();
        let listen = self.listen.flag();
        let output_gain = math::db_to_gain(self.gain.value());
        let channels = buffer.channels().min(MAX_CHANNELS);

        for channel in 0..channels {
            let low_a = &mut self.low_a[channel];
            let low_b = &mut self.low_b[channel];
            let low_c = &mut self.low_c[channel];
            let low_d = &mut self.low_d[channel];
            let floor_a = &mut self.floor_a[channel];
            let floor_b = &mut self.floor_b[channel];
            for sample in buffer.channel_mut(channel) {
                let input = *sample;
                let low = low_b.process(low_a.process(input));
                let driven = low * drive;
                let shaped = Self::shape(driven, blend);
                let mut generated = low_d.process(low_c.process(shaped));
                if floor_active {
                    generated = floor_b.process(floor_a.process(generated));
                }
                let mixed = if listen {
                    generated * amount
                } else {
                    input + generated * amount
                };
                *sample = math::sanitize(mixed * output_gain);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
