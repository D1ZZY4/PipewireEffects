use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

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
    volume: Knob,
    clipping: Knob,
    clipping_range: Knob,
    low: [Biquad; MAX_CHANNELS],
    high: [Biquad; MAX_CHANNELS],
    signature: [f32; 2],
    rate: f32,
}

impl LoudnessEffect {
    pub fn new(params: ParamSet) -> Self {
        Self {
            standard: Knob::new(&params, "standard", 4.0),
            volume: Knob::new(&params, "volume", 0.0),
            clipping: Knob::new(&params, "clipping", 0.0),
            clipping_range: Knob::new(&params, "clipping_range", 6.0),
            low: [Biquad::default(); MAX_CHANNELS],
            high: [Biquad::default(); MAX_CHANNELS],
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
        let channels = buffer.channels().min(MAX_CHANNELS);
        for channel in 0..channels {
            let low = &mut self.low[channel];
            let high = &mut self.high[channel];
            for sample in buffer.channel_mut(channel) {
                let shaped = high.process(low.process(*sample));
                *sample = math::sanitize(shaped.clamp(-ceiling, ceiling));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn tone(frequency: f32, amplitude: f32, frames: usize, offset: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                let phase =
                    std::f32::consts::TAU * frequency * (offset + index) as f32 / RATE as f32;
                *sample = amplitude * phase.sin();
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

    fn measured_gain(frequency: f32, settings: &[(&str, f32)]) -> f32 {
        let params = ParamSet::new(specs());
        for (id, value) in settings {
            params.set_f32(id, *value).unwrap();
        }
        let mut effect = LoudnessEffect::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        let frames = 4_800;
        let mut input = 0.0;
        let mut output = 0.0;
        for block in 0..20 {
            let mut buffer = tone(frequency, 0.25, frames, block * frames);
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
    fn a_full_volume_leaves_the_contour_flat() {
        for frequency in [60.0, 1_000.0, 12_000.0] {
            let gain = measured_gain(frequency, &[("volume", 0.0)]);
            assert!(gain.abs() < 0.5, "{frequency} Hz read {gain} dB");
        }
    }

    #[test]
    fn a_quiet_volume_lifts_the_bass_above_the_midrange() {
        let bass = measured_gain(60.0, &[("volume", -60.0)]);
        let middle = measured_gain(1_000.0, &[("volume", -60.0)]);
        assert!(bass > middle + 3.0, "bass {bass}, middle {middle}");
    }

    #[test]
    fn the_flat_standard_disables_the_contour() {
        let bass = measured_gain(60.0, &[("standard", 0.0), ("volume", -60.0)]);
        assert!(bass.abs() < 0.5, "{bass}");
    }

    #[test]
    fn the_standards_differ_in_how_far_they_lift_the_bass() {
        let iso = measured_gain(60.0, &[("standard", 1.0), ("volume", -60.0)]);
        let fletcher = measured_gain(60.0, &[("standard", 2.0), ("volume", -60.0)]);
        assert!(fletcher > iso + 1.0, "fletcher {fletcher}, iso {iso}");
    }

    #[test]
    fn clipping_holds_the_ceiling() {
        let params = ParamSet::new(specs());
        params.set_bool("clipping", true).unwrap();
        params.set_f32("clipping_range", 6.0).unwrap();
        let mut effect = LoudnessEffect::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        let ceiling = math::db_to_gain(-6.0);
        for block in 0..8 {
            let mut buffer = tone(80.0, 1.0, 4_800, block * 4_800);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.abs() <= ceiling + 1.0e-4, "{}", sample.abs());
                }
            }
        }
    }

    #[test]
    fn the_output_stays_finite_at_the_extremes() {
        let params = ParamSet::new(specs());
        params.set_f32("volume", -83.0).unwrap();
        params.set_bool("clipping", true).unwrap();
        params.set_f32("clipping_range", 0.0).unwrap();
        let mut effect = LoudnessEffect::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        for block in 0..8 {
            let mut buffer = tone(40.0, 2.0, 4_800, block * 4_800);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(
                        sample.is_finite() && sample.abs() <= 1.0 + 1.0e-4,
                        "{sample}"
                    );
                }
            }
        }
    }
}
