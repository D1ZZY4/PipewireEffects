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
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn tone(frequency: f32, frames: usize, offset: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                let phase =
                    std::f32::consts::TAU * frequency * (offset + index) as f32 / RATE as f32;
                *sample = 0.25 * phase.sin();
            }
        }
        buffer
    }

    fn energy(buffer: &AudioBuffer) -> f64 {
        let mut sum = 0.0;
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                sum += f64::from(*sample) * f64::from(*sample);
            }
        }
        sum
    }

    fn measured_gain(frequency: f32, settings: [(&str, f32); 3]) -> f32 {
        let params = ParamSet::new(specs());
        for (id, value) in settings {
            params.set_f32(id, value).unwrap();
        }
        let mut effect = BassLoudness::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        let frames = 4800;
        let mut input = 0.0;
        let mut output = 0.0;
        for block in 0..20 {
            let mut buffer = tone(frequency, frames, block * frames);
            let before = energy(&buffer);
            effect.process(&mut buffer);
            if block >= 16 {
                input += before;
                output += energy(&buffer);
            }
        }
        (10.0 * (output / input).log10()) as f32
    }

    #[test]
    fn a_quiet_source_lifts_the_bass_more_than_the_midrange() {
        let bass = measured_gain(60.0, [("loudness", -50.0), ("output", 0.0), ("link", 0.0)]);
        let middle = measured_gain(
            1_000.0,
            [("loudness", -50.0), ("output", 0.0), ("link", 0.0)],
        );
        assert!(bass > middle + 3.0, "bass {bass}, middle {middle}");
        assert!(bass < 18.5, "{bass}");
    }

    #[test]
    fn a_full_level_source_keeps_the_contour_flat() {
        for frequency in [60.0, 1_000.0, 12_000.0] {
            let gain = measured_gain(
                frequency,
                [("loudness", 0.0), ("output", 0.0), ("link", 0.0)],
            );
            assert!(gain.abs() < 0.6, "{frequency} Hz read {gain} dB");
        }
    }

    #[test]
    fn the_output_control_trims_the_level() {
        let gain = measured_gain(
            1_000.0,
            [("loudness", 0.0), ("output", -6.0), ("link", 0.0)],
        );
        assert!((gain + 6.0).abs() < 0.3, "{gain}");
    }

    #[test]
    fn the_link_control_lowers_the_output_as_the_source_falls() {
        let linked = measured_gain(
            1_000.0,
            [("loudness", -50.0), ("output", 0.0), ("link", -9.1)],
        );
        let open = measured_gain(
            1_000.0,
            [("loudness", -50.0), ("output", 0.0), ("link", 0.0)],
        );
        assert!(linked < open - 4.0, "linked {linked}, open {open}");
    }

    #[test]
    fn the_output_stays_finite_at_the_extremes() {
        let params = ParamSet::new(specs());
        params.set_f32("loudness", -60.0).unwrap();
        params.set_f32("output", -60.0).unwrap();
        params.set_f32("link", -60.0).unwrap();
        let mut effect = BassLoudness::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        for block in 0..4 {
            let mut buffer = tone(40.0, 4800, block * 4800);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 4.0, "{sample}");
                }
            }
        }
    }
}
