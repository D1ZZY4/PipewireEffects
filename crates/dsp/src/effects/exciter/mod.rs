use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const RESONANCE: f32 = std::f32::consts::FRAC_1_SQRT_2;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("amount", "Amount", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Exciter")
            .build(),
        ParamSpec::float("harmonics", "Drive", ParamUnit::None)
            .range(0.1, 10.0, 8.5)
            .step(0.1)
            .group("Exciter")
            .build(),
        ParamSpec::float("blend", "Blend", ParamUnit::None)
            .range(-10.0, 10.0, 0.0)
            .step(0.1)
            .group("Exciter")
            .build(),
        ParamSpec::float("scope", "Scope", ParamUnit::Hertz)
            .range(2_000.0, 12_000.0, 7_500.0)
            .logarithmic()
            .group("Band")
            .build(),
        ParamSpec::float("ceil", "Ceiling", ParamUnit::Hertz)
            .range(10_000.0, 20_000.0, 16_000.0)
            .logarithmic()
            .group("Band")
            .build(),
        ParamSpec::boolean("ceil_active", "Ceiling active", false).group("Band"),
        ParamSpec::boolean("listen", "Harmonics only", false).group("Output"),
    ]
}

pub struct Exciter {
    amount: Knob,
    harmonics: Knob,
    blend: Knob,
    scope: Knob,
    ceil: Knob,
    ceil_active: Knob,
    listen: Knob,
    high_pass: [Biquad; MAX_CHANNELS],
    low_pass: [Biquad; MAX_CHANNELS],
    rate: f32,
    signature: [f32; 2],
}

impl Exciter {
    pub fn new(params: ParamSet) -> Self {
        Self {
            amount: Knob::new(&params, "amount", 0.0),
            harmonics: Knob::new(&params, "harmonics", 8.5),
            blend: Knob::new(&params, "blend", 0.0),
            scope: Knob::new(&params, "scope", 7_500.0),
            ceil: Knob::new(&params, "ceil", 16_000.0),
            ceil_active: Knob::new(&params, "ceil_active", 0.0),
            listen: Knob::new(&params, "listen", 0.0),
            high_pass: [Biquad::default(); MAX_CHANNELS],
            low_pass: [Biquad::default(); MAX_CHANNELS],
            rate: 48_000.0,
            signature: [f32::NAN; 2],
        }
    }

    fn refresh(&mut self, scope: f32, ceil: f32) {
        if self.signature == [scope, ceil] {
            return;
        }
        self.signature = [scope, ceil];
        for channel in 0..MAX_CHANNELS {
            self.high_pass[channel] = Biquad::new(Coefficients::design(
                FilterKind::HighPass,
                self.rate,
                scope,
                RESONANCE,
                0.0,
            ));
            self.low_pass[channel] = Biquad::new(Coefficients::design(
                FilterKind::LowPass,
                self.rate,
                ceil,
                RESONANCE,
                0.0,
            ));
        }
    }
}

impl Effect for Exciter {
    fn id(&self) -> &'static str {
        "exciter"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(1) as f32;
        self.signature = [f32::NAN; 2];
        self.reset();
    }

    fn reset(&mut self) {
        for stage in &mut self.high_pass {
            stage.reset();
        }
        for stage in &mut self.low_pass {
            stage.reset();
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let scope = self.scope.value();
        self.refresh(scope, self.ceil.value());
        let amount = math::db_to_gain(self.amount.value());
        let drive = self.harmonics.value().max(0.1);
        let blend = ((self.blend.value() + 10.0) / 20.0).clamp(0.0, 1.0);
        let active = self.ceil_active.flag();
        let listen = self.listen.flag();
        let channels = buffer.channels().min(MAX_CHANNELS);

        for channel in 0..channels {
            let high_pass = &mut self.high_pass[channel];
            let low_pass = &mut self.low_pass[channel];
            for sample in buffer.channel_mut(channel) {
                let input = *sample;
                let mut band = high_pass.process(input);
                if active {
                    band = low_pass.process(band);
                }
                let driven = band * drive;
                let saturated = driven / (1.0 + driven.abs());
                let generated = saturated * (1.0 - blend) + band * blend;
                *sample = math::sanitize(if listen {
                    generated
                } else {
                    input + amount * generated
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
