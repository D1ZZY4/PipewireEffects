use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

mod spectral;

use spectral::SpectralContour;

const STANDARDS: usize = 5;
const SHELF_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
const MAX_LEVEL_DB: f32 = 83.0;

const STANDARD_LABELS: [&str; STANDARDS] = [
    mark("Flat"),
    mark("ISO 226:2003"),
    mark("Fletcher-Munson"),
    mark("Robinson-Dadson"),
    mark("ISO 226:2023"),
];

const MODE_LABELS: [&str; 2] = [mark("FFT"), mark("IIR")];

const SHAPES: [(f32, f32, f32, f32, f32, f32); STANDARDS] = [
    (0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
    (120.0, 18.0, 40.0, 6_000.0, 8.0, 60.0),
    (150.0, 24.0, 45.0, 8_000.0, 10.0, 70.0),
    (130.0, 20.0, 42.0, 7_000.0, 9.0, 65.0),
    (125.0, 19.0, 41.0, 6_500.0, 8.5, 62.0),
];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("standard", "Standard", &STANDARD_LABELS, 4).group("Contour"),
        ParamSpec::choice("mode", "Mode", &MODE_LABELS, 1).group("Contour"),
        ParamSpec::float("volume", "Output volume", ParamUnit::Decibel)
            .range(-83.0, 7.0, 0.0)
            .step(0.1)
            .group("Contour")
            .build(),
        ParamSpec::boolean("clipping", "Clipping", false).group("Protection"),
        ParamSpec::float("clipping_range", "Clipping range", ParamUnit::Decibel)
            .range(0.0, 24.0, 6.0)
            .step(0.5)
            .group("Protection")
            .build(),
    ]
}

pub struct LoudnessEffect {
    standard: Knob,
    mode: Knob,
    volume: Knob,
    clipping: Knob,
    clipping_range: Knob,
    low: [Biquad; MAX_CHANNELS],
    high: [Biquad; MAX_CHANNELS],
    low_coefficients: Coefficients,
    high_coefficients: Coefficients,
    spectral: SpectralContour,
    signature: [f32; 2],
    rate: f32,
}

impl LoudnessEffect {
    pub fn new(params: ParamSet) -> Self {
        Self {
            standard: Knob::new(&params, "standard", 4.0),
            mode: Knob::new(&params, "mode", 1.0),
            volume: Knob::new(&params, "volume", 0.0),
            clipping: Knob::new(&params, "clipping", 0.0),
            clipping_range: Knob::new(&params, "clipping_range", 6.0),
            low: [Biquad::default(); MAX_CHANNELS],
            high: [Biquad::default(); MAX_CHANNELS],
            low_coefficients: Coefficients::identity(),
            high_coefficients: Coefficients::identity(),
            spectral: SpectralContour::new(),
            signature: [f32::NAN; 2],
            rate: 48_000.0,
        }
    }

    fn contour_gain(level_db: f32, ceiling_db: f32, scale_db: f32) -> f32 {
        if ceiling_db <= 0.0 {
            return 0.0;
        }
        ceiling_db * (1.0 - (-level_db / scale_db).exp())
    }

    fn redesign(&mut self, standard: usize, volume: f32) {
        let signature = [standard as f32, volume];
        if signature == self.signature {
            return;
        }
        self.signature = signature;
        let shape = SHAPES[standard.min(STANDARDS - 1)];
        let level = (-volume).clamp(0.0, MAX_LEVEL_DB);
        let low = Coefficients::design(
            FilterKind::LowShelf,
            self.rate,
            shape.0.max(1.0),
            SHELF_Q,
            Self::contour_gain(level, shape.1, shape.2),
        );
        let high = Coefficients::design(
            FilterKind::HighShelf,
            self.rate,
            shape.3.max(1.0),
            SHELF_Q,
            Self::contour_gain(level, shape.4, shape.5),
        );
        for biquad in &mut self.low {
            biquad.set_coefficients(low);
        }
        for biquad in &mut self.high {
            biquad.set_coefficients(high);
        }
        self.low_coefficients = low;
        self.high_coefficients = high;
        self.spectral.update(low, high, self.rate);
    }
}

fn clamp_buffer(buffer: &mut AudioBuffer, ceiling: f32) {
    if ceiling.is_infinite() {
        return;
    }
    let channels = buffer.channels().min(MAX_CHANNELS);
    for channel in 0..channels {
        for sample in buffer.channel_mut(channel) {
            *sample = math::sanitize(sample.clamp(-ceiling, ceiling));
        }
    }
}

impl Effect for LoudnessEffect {
    fn id(&self) -> &'static str {
        "loudness"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(8_000) as f32;
        self.signature = [f32::NAN; 2];
        self.reset();
    }

    fn reset(&mut self) {
        for biquad in &mut self.low {
            biquad.reset();
        }
        for biquad in &mut self.high {
            biquad.reset();
        }
        self.spectral.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let standard = self.standard.index();
        let volume = self.volume.value();
        self.redesign(standard, volume);
        let ceiling = if self.clipping.flag() {
            math::db_to_gain(-self.clipping_range.value().abs())
        } else {
            f32::INFINITY
        };
        if self.mode.index() == 0 {
            self.spectral.process(buffer);
        } else {
            let channels = buffer.channels().min(MAX_CHANNELS);
            for channel in 0..channels {
                let low = &mut self.low[channel];
                let high = &mut self.high[channel];
                for sample in buffer.channel_mut(channel) {
                    *sample = math::sanitize(high.process(low.process(*sample)));
                }
            }
        }
        clamp_buffer(buffer, ceiling);
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
