use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub const BAND_COUNT: usize = 10;

pub const BAND_FREQUENCIES: [f32; BAND_COUNT] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

const BAND_IDS: [&str; BAND_COUNT] = [
    "band1_gain",
    "band2_gain",
    "band3_gain",
    "band4_gain",
    "band5_gain",
    "band6_gain",
    "band7_gain",
    "band8_gain",
    "band9_gain",
    "band10_gain",
];

const BAND_LABELS: [&str; BAND_COUNT] = [
    "31 Hz", "62 Hz", "125 Hz", "250 Hz", "500 Hz", "1 kHz", "2 kHz", "4 kHz", "8 kHz", "16 kHz",
];

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(BAND_COUNT + 2);
    for band in 0..BAND_COUNT {
        specs.push(
            ParamSpec::float(BAND_IDS[band], BAND_LABELS[band], ParamUnit::Decibel)
                .range(-24.0, 24.0, 0.0)
                .step(0.1)
                .group("Bands")
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

    pub fn gain_values(&self) -> [f32; BAND_COUNT] {
        std::array::from_fn(|band| self.gains[band].value())
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
mod tests {
    use super::*;

    fn tone(frequency: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample = (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    #[test]
    fn a_flat_equaliser_is_transparent() {
        let params = ParamSet::new(specs());
        let mut effect = Equalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(1_000.0, 1024);
        let reference = buffer.channel(0).to_vec();
        effect.process(&mut buffer);
        for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
            assert!(
                (processed - original).abs() < 1.0e-3,
                "{processed} vs {original}"
            );
        }
    }

    #[test]
    fn boosting_one_band_raises_that_frequency() {
        let params = ParamSet::new(specs());
        params.set_f32("band6_gain", 8.0).unwrap();
        let mut effect = Equalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut peak = 0.0f32;
        for _ in 0..64 {
            let mut buffer = tone(1_000.0, 1024);
            effect.process(&mut buffer);
            peak = peak.max(buffer.peak());
        }
        let expected = math::db_to_gain(8.0);
        assert!(
            (peak - expected).abs() < 0.3,
            "peak {peak} expected {expected}"
        );
    }

    #[test]
    fn boosting_one_band_does_not_leak_into_another() {
        let params = ParamSet::new(specs());
        params.set_f32("band8_gain", 12.0).unwrap();
        let mut effect = Equalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut peak = 0.0f32;
        for _ in 0..64 {
            let mut buffer = tone(125.0, 1024);
            effect.process(&mut buffer);
            peak = peak.max(buffer.peak());
        }
        assert!(peak < 1.1, "leakage {peak}");
    }

    #[test]
    fn analytic_response_matches_the_measured_response() {
        let params = ParamSet::new(specs());
        params.set_f32("band4_gain", 6.0).unwrap();
        let mut effect = Equalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut peak = 0.0f32;
        for _ in 0..64 {
            let mut buffer = tone(250.0, 1024);
            effect.process(&mut buffer);
            peak = peak.max(buffer.peak());
        }
        let mut gains = [0.0f32; BAND_COUNT];
        gains[3] = 6.0;
        let predicted = math::db_to_gain(response_db(48_000.0, &gains, 1.414, 250.0));
        assert!(
            (peak - predicted).abs() < 0.05,
            "measured {peak} predicted {predicted}"
        );
    }

    #[test]
    fn output_gain_is_applied() {
        let params = ParamSet::new(specs());
        params.set_f32("output_gain", -6.0).unwrap();
        let mut effect = Equalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(1_000.0, 1024);
        effect.process(&mut buffer);
        assert!((buffer.peak() - math::db_to_gain(-6.0)).abs() < 0.01);
    }
}
