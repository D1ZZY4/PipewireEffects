use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub const BAND_COUNT: usize = 30;

pub const BAND_FREQUENCIES: [f32; BAND_COUNT] = [
    20.0, 25.0, 31.25, 40.0, 50.0, 62.5, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1_000.0, 1_250.0, 1_600.0, 2_000.0, 2_500.0, 3_150.0, 4_000.0, 5_000.0,
    6_300.0, 8_000.0, 10_000.0, 12_500.0, 16_000.0,
];

pub const BAND_IDS: [&str; BAND_COUNT] = [
    "band11_gain",
    "band12_gain",
    "band1_gain",
    "band13_gain",
    "band14_gain",
    "band2_gain",
    "band15_gain",
    "band16_gain",
    "band3_gain",
    "band17_gain",
    "band18_gain",
    "band4_gain",
    "band19_gain",
    "band20_gain",
    "band5_gain",
    "band21_gain",
    "band22_gain",
    "band6_gain",
    "band23_gain",
    "band24_gain",
    "band7_gain",
    "band25_gain",
    "band26_gain",
    "band8_gain",
    "band27_gain",
    "band28_gain",
    "band9_gain",
    "band29_gain",
    "band30_gain",
    "band10_gain",
];

pub const BAND_LABELS: [&str; BAND_COUNT] = [
    mark("20 Hz"),
    mark("25 Hz"),
    mark("31 Hz"),
    mark("40 Hz"),
    mark("50 Hz"),
    mark("62 Hz"),
    mark("80 Hz"),
    mark("100 Hz"),
    mark("125 Hz"),
    mark("160 Hz"),
    mark("200 Hz"),
    mark("250 Hz"),
    mark("315 Hz"),
    mark("400 Hz"),
    mark("500 Hz"),
    mark("630 Hz"),
    mark("800 Hz"),
    mark("1 kHz"),
    mark("1.25 kHz"),
    mark("1.6 kHz"),
    mark("2 kHz"),
    mark("2.5 kHz"),
    mark("3.15 kHz"),
    mark("4 kHz"),
    mark("5 kHz"),
    mark("6.3 kHz"),
    mark("8 kHz"),
    mark("10 kHz"),
    mark("12.5 kHz"),
    mark("16 kHz"),
];

fn band_group(frequency: f32) -> &'static str {
    if frequency < 250.0 {
        mark("Lows")
    } else if frequency < 2_000.0 {
        mark("Mids")
    } else {
        mark("Highs")
    }
}

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(BAND_COUNT + 2);
    for band in 0..BAND_COUNT {
        let frequency = BAND_FREQUENCIES[band];
        specs.push(
            ParamSpec::float(BAND_IDS[band], BAND_LABELS[band], ParamUnit::Decibel)
                .range(-24.0, 24.0, 0.0)
                .step(0.1)
                .group(band_group(frequency))
                .build(),
        );
    }
    specs.push(
        ParamSpec::float("bandwidth", "Bandwidth", ParamUnit::None)
            .range(0.3, 6.0, 1.414)
            .step(0.01)
            .group("Tone")
            .build(),
    );
    specs.push(
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Tone")
            .build(),
    );
    specs
}

pub fn response_db(rate: f32, gains: &[f32; BAND_COUNT], bandwidth: f32, frequency: f32) -> f32 {
    let mut total = 0.0f32;
    for band in 0..BAND_COUNT {
        let coefficients = Coefficients::design(
            FilterKind::Peaking,
            rate,
            BAND_FREQUENCIES[band],
            bandwidth,
            gains[band],
        );
        let magnitude = coefficients.magnitude_at(rate, frequency).max(1.0e-9);
        total += 20.0 * magnitude.log10();
    }
    total
}

pub struct Equalizer {
    bands: [[Biquad; BAND_COUNT]; MAX_CHANNELS],
    gains: [Knob; BAND_COUNT],
    bandwidth: Knob,
    output_gain: Knob,
    signature: [f32; BAND_COUNT + 2],
    output_smoothed: math::Smoothed,
    rate: f32,
    dirty: bool,
}

impl Equalizer {
    pub fn new(params: ParamSet) -> Self {
        let gains = std::array::from_fn(|band| Knob::new(&params, BAND_IDS[band], 0.0));
        Self {
            bands: [[Biquad::default(); BAND_COUNT]; MAX_CHANNELS],
            gains,
            bandwidth: Knob::new(&params, "bandwidth", 1.414),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            signature: [f32::NAN; BAND_COUNT + 2],
            output_smoothed: math::Smoothed::new(1.0),
            rate: 48_000.0,
            dirty: true,
        }
    }

    fn refresh(&mut self) {
        let bandwidth = self.bandwidth.value();
        for channel in 0..MAX_CHANNELS {
            for (band, (frequency, knob)) in
                BAND_FREQUENCIES.iter().zip(self.gains.iter()).enumerate()
            {
                let coefficients = Coefficients::design(
                    FilterKind::Peaking,
                    self.rate,
                    *frequency,
                    bandwidth,
                    knob.value(),
                );
                self.bands[channel][band].set_coefficients(coefficients);
            }
        }
    }

    fn signature_now(&self) -> [f32; BAND_COUNT + 2] {
        let mut signature = [0.0f32; BAND_COUNT + 2];
        for (slot, knob) in signature.iter_mut().zip(self.gains.iter()).take(BAND_COUNT) {
            *slot = knob.value();
        }
        signature[BAND_COUNT] = self.bandwidth.value();
        signature[BAND_COUNT + 1] = self.output_gain.value();
        signature
    }
}

impl Effect for Equalizer {
    fn id(&self) -> &'static str {
        "equalizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.output_smoothed.set_time_constant(20.0, format.rate());
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.bands {
            for band in channel {
                band.reset();
            }
        }
        self.output_smoothed
            .snap(math::db_to_gain(self.output_gain.value()));
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let signature = self.signature_now();
        if self.dirty || signature != self.signature {
            self.signature = signature;
            self.dirty = false;
            self.refresh();
        }
        let channels = buffer.channels().min(MAX_CHANNELS);
        for channel in 0..channels {
            let stages = &mut self.bands[channel];
            for sample in buffer.channel_mut(channel) {
                let mut value = *sample;
                for stage in stages.iter_mut() {
                    value = stage.process(value);
                }
                *sample = value;
            }
        }
        self.output_smoothed
            .set_target(math::db_to_gain(signature[BAND_COUNT + 1]));
        let gain = self.output_smoothed.advance();
        if (gain - 1.0).abs() > f32::EPSILON {
            for channel in buffer.channels_mut() {
                for sample in channel.iter_mut() {
                    *sample = math::sanitize(*sample * gain);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
