use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::knob::Knob;

use super::equalizer::{BAND_COUNT, BAND_FREQUENCIES, BAND_LABELS};

const MID_IDS: [&str; BAND_COUNT] = [
    "mid_band11_gain",
    "mid_band12_gain",
    "mid_band1_gain",
    "mid_band13_gain",
    "mid_band14_gain",
    "mid_band2_gain",
    "mid_band15_gain",
    "mid_band16_gain",
    "mid_band3_gain",
    "mid_band17_gain",
    "mid_band18_gain",
    "mid_band4_gain",
    "mid_band19_gain",
    "mid_band20_gain",
    "mid_band5_gain",
    "mid_band21_gain",
    "mid_band22_gain",
    "mid_band6_gain",
    "mid_band23_gain",
    "mid_band24_gain",
    "mid_band7_gain",
    "mid_band25_gain",
    "mid_band26_gain",
    "mid_band8_gain",
    "mid_band27_gain",
    "mid_band28_gain",
    "mid_band9_gain",
    "mid_band29_gain",
    "mid_band30_gain",
    "mid_band10_gain",
];

const SIDE_IDS: [&str; BAND_COUNT] = [
    "side_band11_gain",
    "side_band12_gain",
    "side_band1_gain",
    "side_band13_gain",
    "side_band14_gain",
    "side_band2_gain",
    "side_band15_gain",
    "side_band16_gain",
    "side_band3_gain",
    "side_band17_gain",
    "side_band18_gain",
    "side_band4_gain",
    "side_band19_gain",
    "side_band20_gain",
    "side_band5_gain",
    "side_band21_gain",
    "side_band22_gain",
    "side_band6_gain",
    "side_band23_gain",
    "side_band24_gain",
    "side_band7_gain",
    "side_band25_gain",
    "side_band26_gain",
    "side_band8_gain",
    "side_band27_gain",
    "side_band28_gain",
    "side_band9_gain",
    "side_band29_gain",
    "side_band30_gain",
    "side_band10_gain",
];

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(BAND_COUNT * 2 + 4);
    for band in 0..BAND_COUNT {
        specs.push(band_spec(
            MID_IDS[band],
            BAND_LABELS[band],
            mark("Mid bands"),
        ));
    }
    for band in 0..BAND_COUNT {
        specs.push(band_spec(
            SIDE_IDS[band],
            BAND_LABELS[band],
            mark("Side bands"),
        ));
    }
    specs.push(
        ParamSpec::float("bandwidth", "Bandwidth", ParamUnit::None)
            .range(0.3, 6.0, 1.414)
            .step(0.01)
            .group("Tone")
            .build(),
    );
    specs.push(
        ParamSpec::float("mid_gain", "Mid gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Tone")
            .build(),
    );
    specs.push(
        ParamSpec::float("side_gain", "Side gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Tone")
            .build(),
    );
    specs.push(
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    );
    specs
}

fn band_spec(id: &'static str, label: &'static str, group: &'static str) -> ParamSpec {
    ParamSpec::float(id, label, ParamUnit::Decibel)
        .range(-24.0, 24.0, 0.0)
        .step(0.1)
        .group(group)
        .build()
}

pub struct MidSideEqualizer {
    mid_gains: [Knob; BAND_COUNT],
    side_gains: [Knob; BAND_COUNT],
    bandwidth: Knob,
    mid_gain: Knob,
    side_gain: Knob,
    output_gain: Knob,
    mid_bands: [Biquad; BAND_COUNT],
    side_bands: [Biquad; BAND_COUNT],
    signature: [f32; BAND_COUNT * 2 + 1],
    rate: f32,
    dirty: bool,
}

impl MidSideEqualizer {
    pub fn new(params: ParamSet) -> Self {
        let mid_gains = std::array::from_fn(|band| Knob::new(&params, MID_IDS[band], 0.0));
        let side_gains = std::array::from_fn(|band| Knob::new(&params, SIDE_IDS[band], 0.0));
        Self {
            mid_gains,
            side_gains,
            bandwidth: Knob::new(&params, "bandwidth", 1.414),
            mid_gain: Knob::new(&params, "mid_gain", 0.0),
            side_gain: Knob::new(&params, "side_gain", 0.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            mid_bands: [Biquad::default(); BAND_COUNT],
            side_bands: [Biquad::default(); BAND_COUNT],
            signature: [f32::NAN; BAND_COUNT * 2 + 1],
            rate: 48_000.0,
            dirty: true,
        }
    }

    fn signature_now(&self) -> [f32; BAND_COUNT * 2 + 1] {
        let mut signature = [0.0f32; BAND_COUNT * 2 + 1];
        for (slot, knob) in signature.iter_mut().zip(self.mid_gains.iter()) {
            *slot = knob.value();
        }
        for (slot, knob) in signature[BAND_COUNT..]
            .iter_mut()
            .zip(self.side_gains.iter())
        {
            *slot = knob.value();
        }
        signature[BAND_COUNT * 2] = self.bandwidth.value();
        signature
    }

    fn refresh(&mut self, bandwidth: f32) {
        for (band, frequency) in BAND_FREQUENCIES.iter().enumerate() {
            let mid = Coefficients::design(
                FilterKind::Peaking,
                self.rate,
                *frequency,
                bandwidth,
                self.mid_gains[band].value(),
            );
            self.mid_bands[band].set_coefficients(mid);
            let side = Coefficients::design(
                FilterKind::Peaking,
                self.rate,
                *frequency,
                bandwidth,
                self.side_gains[band].value(),
            );
            self.side_bands[band].set_coefficients(side);
        }
    }
}

impl Effect for MidSideEqualizer {
    fn id(&self) -> &'static str {
        "midside_equalizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(1) as f32;
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for stage in &mut self.mid_bands {
            stage.reset();
        }
        for stage in &mut self.side_bands {
            stage.reset();
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let signature = self.signature_now();
        if self.dirty || signature != self.signature {
            self.signature = signature;
            self.dirty = false;
            self.refresh(signature[BAND_COUNT * 2]);
        }
        if buffer.channels() < 2 {
            return;
        }
        let mid_trim = math::db_to_gain(self.mid_gain.value());
        let side_trim = math::db_to_gain(self.side_gain.value());
        let output = math::db_to_gain(self.output_gain.value());
        let frames = buffer.frames();

        for frame in 0..frames {
            let left = buffer.channel(0)[frame];
            let right = buffer.channel(1)[frame];
            let mut middle = 0.5 * (left + right);
            let mut side = 0.5 * (left - right);
            for stage in &mut self.mid_bands {
                middle = stage.process(middle);
            }
            for stage in &mut self.side_bands {
                side = stage.process(side);
            }
            middle *= mid_trim;
            side *= side_trim;
            buffer.channel_mut(0)[frame] = math::sanitize((middle + side) * output);
            buffer.channel_mut(1)[frame] = math::sanitize((middle - side) * output);
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
