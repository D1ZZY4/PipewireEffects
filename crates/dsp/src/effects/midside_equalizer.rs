use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::knob::Knob;

use super::equalizer::{BAND_COUNT, BAND_FREQUENCIES};

const BAND_LABELS: [&str; BAND_COUNT] = [
    mark("31 Hz"),
    mark("62 Hz"),
    mark("125 Hz"),
    mark("250 Hz"),
    mark("500 Hz"),
    mark("1 kHz"),
    mark("2 kHz"),
    mark("4 kHz"),
    mark("8 kHz"),
    mark("16 kHz"),
];

const MID_IDS: [&str; BAND_COUNT] = [
    "mid_band1_gain",
    "mid_band2_gain",
    "mid_band3_gain",
    "mid_band4_gain",
    "mid_band5_gain",
    "mid_band6_gain",
    "mid_band7_gain",
    "mid_band8_gain",
    "mid_band9_gain",
    "mid_band10_gain",
];

const SIDE_IDS: [&str; BAND_COUNT] = [
    "side_band1_gain",
    "side_band2_gain",
    "side_band3_gain",
    "side_band4_gain",
    "side_band5_gain",
    "side_band6_gain",
    "side_band7_gain",
    "side_band8_gain",
    "side_band9_gain",
    "side_band10_gain",
];

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(BAND_COUNT * 2 + 4);
    for band in 0..BAND_COUNT {
        specs.push(band_spec(MID_IDS[band], BAND_LABELS[band], "Mid bands"));
    }
    for band in 0..BAND_COUNT {
        specs.push(band_spec(SIDE_IDS[band], BAND_LABELS[band], "Side bands"));
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
mod tests {
    use super::*;

    fn settings(pairs: &[(&str, f32)]) -> ParamSet {
        let params = ParamSet::new(specs());
        for (key, value) in pairs {
            params.set_f32(key, *value).unwrap();
        }
        params
    }

    fn tone(left: f32, right: f32, frequency: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for index in 0..frames {
            let value = (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
            buffer.channel_mut(0)[index] = value * left;
            buffer.channel_mut(1)[index] = value * right;
        }
        buffer
    }

    fn settled(pairs: &[(&str, f32)], left: f32, right: f32, frequency: f32) -> AudioBuffer {
        let mut effect = MidSideEqualizer::new(settings(pairs));
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut peak = 0.0f32;
        let mut last = tone(left, right, frequency, 2_048);
        for _ in 0..32 {
            last = tone(left, right, frequency, 2_048);
            effect.process(&mut last);
            peak = peak.max(last.peak());
        }
        assert!(peak.is_finite());
        last
    }

    #[test]
    fn a_flat_curve_is_transparent() {
        let processed = settled(&[], 0.5, -0.5, 1_000.0);
        let reference = tone(0.5, -0.5, 1_000.0, 2_048);
        for channel in 0..2 {
            for (processed, original) in processed
                .channel(channel)
                .iter()
                .zip(reference.channel(channel))
            {
                assert!(
                    (processed - original).abs() < 1.0e-3,
                    "{processed} {original}"
                );
            }
        }
    }

    #[test]
    fn a_mid_band_boost_changes_a_mono_signal() {
        let processed = settled(&[("mid_band6_gain", 8.0)], 0.4, 0.4, 1_000.0);
        assert!(processed.peak() > 0.4 + 0.2, "{}", processed.peak());
    }

    #[test]
    fn a_side_band_boost_leaves_a_mono_signal_alone() {
        let processed = settled(&[("side_band6_gain", 12.0)], 0.4, 0.4, 1_000.0);
        assert!(
            (processed.peak() - 0.4).abs() < 0.05,
            "{}",
            processed.peak()
        );
    }

    #[test]
    fn the_side_gain_widens_and_narrows_the_image() {
        let wide = settled(&[("side_gain", 6.0)], 0.3, -0.3, 500.0);
        let narrow = settled(&[("side_gain", -6.0)], 0.3, -0.3, 500.0);
        assert!(wide.peak() > narrow.peak() * 1.5);
    }

    #[test]
    fn the_output_gain_is_applied() {
        let processed = settled(&[("output_gain", -6.0)], 0.4, -0.4, 800.0);
        assert!((processed.peak() - 0.4 * math::db_to_gain(-6.0)).abs() < 0.02);
    }
}
