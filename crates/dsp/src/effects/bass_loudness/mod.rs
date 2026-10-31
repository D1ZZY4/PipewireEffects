use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const LOW_SHELF_HZ: f32 = 120.0;
const HIGH_SHELF_HZ: f32 = 6_000.0;
const LOW_MAX_DB: f32 = 18.0;
const HIGH_MAX_DB: f32 = 8.0;
const LOW_SCALE_DB: f32 = 40.0;
const HIGH_SCALE_DB: f32 = 60.0;
const LINK_SCALE_DB: f32 = 40.0;
const SHELF_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
const MIN_DB: f32 = -60.0;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("loudness", "Loudness", ParamUnit::Decibel)
            .range(MIN_DB, 0.0, -3.1)
            .step(0.1)
            .group("Contour")
            .build(),
        ParamSpec::float("output", "Output", ParamUnit::Decibel)
            .range(MIN_DB, 0.0, -6.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("link", "Link", ParamUnit::Decibel)
            .range(MIN_DB, 0.0, -9.1)
            .step(0.1)
            .group("Output")
            .build(),
    ]
}

pub struct BassLoudness {
    loudness: Knob,
    output: Knob,
    link: Knob,
    low: [Biquad; MAX_CHANNELS],
    high: [Biquad; MAX_CHANNELS],
    signature: f32,
    rate: f32,
}

impl BassLoudness {
    pub fn new(params: ParamSet) -> Self {
        Self {
            loudness: Knob::new(&params, "loudness", -3.1),
            output: Knob::new(&params, "output", -6.0),
            link: Knob::new(&params, "link", -9.1),
            low: [Biquad::default(); MAX_CHANNELS],
            high: [Biquad::default(); MAX_CHANNELS],
            signature: f32::NAN,
            rate: 48_000.0,
        }
    }

    fn contour(level_db: f32, ceiling_db: f32, scale_db: f32) -> f32 {
        ceiling_db * (1.0 - (-level_db / scale_db).exp())
    }

    fn update_coefficients(&mut self, loudness: f32) {
        let level = (-loudness).clamp(0.0, -MIN_DB);
        let low = Coefficients::design(
            FilterKind::LowShelf,
            self.rate,
            LOW_SHELF_HZ,
            SHELF_Q,
            Self::contour(level, LOW_MAX_DB, LOW_SCALE_DB),
        );
        let high = Coefficients::design(
            FilterKind::HighShelf,
            self.rate,
            HIGH_SHELF_HZ,
            SHELF_Q,
            Self::contour(level, HIGH_MAX_DB, HIGH_SCALE_DB),
        );
        for biquad in &mut self.low {
            biquad.set_coefficients(low);
        }
        for biquad in &mut self.high {
            biquad.set_coefficients(high);
        }
    }

    fn output_gain(&self) -> f32 {
        let level = (-self.loudness.value()).clamp(0.0, -MIN_DB);
        let published = self.link.value() * Self::contour(level, 1.0, LINK_SCALE_DB);
        math::db_to_gain(self.output.value() + published)
    }
}

impl Effect for BassLoudness {
    fn id(&self) -> &'static str {
        "bass_loudness"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(8_000) as f32;
        self.signature = f32::NAN;
        self.reset();
    }

    fn reset(&mut self) {
        for biquad in &mut self.low {
            biquad.reset();
        }
        for biquad in &mut self.high {
            biquad.reset();
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let loudness = self.loudness.value();
        if loudness != self.signature {
            self.signature = loudness;
            self.update_coefficients(loudness);
        }
        let gain = self.output_gain();
        let channels = buffer.channels().min(MAX_CHANNELS);
        for channel in 0..channels {
            let low = &mut self.low[channel];
            let high = &mut self.high[channel];
            let plan = buffer.channel_mut(channel);
            for sample in plan.iter_mut() {
                *sample = math::sanitize(high.process(low.process(*sample)) * gain);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
