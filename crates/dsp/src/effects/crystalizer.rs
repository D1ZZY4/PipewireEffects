use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const BANDS: usize = 13;
const SPLITS: [f32; BANDS] = [
    32.0, 64.0, 128.0, 256.0, 512.0, 1_024.0, 2_048.0, 4_096.0, 6_000.0, 8_000.0, 12_000.0,
    16_000.0, 20_000.0,
];

const LABELS: [&str; BANDS] = [
    mark("Below 32 Hz"),
    mark("32 to 64 Hz"),
    mark("64 to 128 Hz"),
    mark("128 to 256 Hz"),
    mark("256 to 512 Hz"),
    mark("512 Hz to 1 kHz"),
    mark("1 to 2 kHz"),
    mark("2 to 4 kHz"),
    mark("4 to 6 kHz"),
    mark("6 to 8 kHz"),
    mark("8 to 12 kHz"),
    mark("12 to 16 kHz"),
    mark("Above 16 kHz"),
];

macro_rules! band_ids {
    ($($n:literal),* $(,)?) => {
        [
            $([
                concat!("band", $n, "_intensity"),
                concat!("band", $n, "_mute"),
                concat!("band", $n, "_bypass"),
            ]),*
        ]
    };
}

const BAND_IDS: [[&str; 3]; BANDS] = band_ids![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,];

const INTENSITY: usize = 0;
const MUTE: usize = 1;
const BYPASS: usize = 2;

fn group_for(band: usize) -> &'static str {
    match band {
        0..=3 => "Low",
        4..=7 => "Mid",
        _ => "High",
    }
}

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(1 + BANDS * 3);
    for band in 0..BANDS {
        let group = group_for(band);
        specs.push(
            ParamSpec::float(BAND_IDS[band][INTENSITY], LABELS[band], ParamUnit::Decibel)
                .range(-40.0, 32.0, 0.0)
                .step(0.5)
                .group(group)
                .build(),
        );
        specs.push(ParamSpec::boolean(BAND_IDS[band][MUTE], "Mute", false).group(group));
        specs.push(ParamSpec::boolean(BAND_IDS[band][BYPASS], "Bypass", false).group(group));
    }
    specs.push(
        ParamSpec::float("gain", "Output gain", ParamUnit::Decibel)
            .range(-12.0, 12.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    );
    specs
}

pub struct Crystalizer {
    gain: Knob,
    bands: [[Knob; 3]; BANDS],
    filters: [[Biquad; BANDS]; MAX_CHANNELS],
    fast: [[f32; BANDS]; MAX_CHANNELS],
    slow: [[f32; BANDS]; MAX_CHANNELS],
    fast_coefficient: f32,
    slow_coefficient: f32,
    rate: u32,
}

impl Crystalizer {
    pub fn new(params: ParamSet) -> Self {
        Self {
            gain: Knob::new(&params, "gain", 0.0),
            bands: std::array::from_fn(|band| {
                std::array::from_fn(|param| Knob::new(&params, BAND_IDS[band][param], 0.0))
            }),
            filters: [[Biquad::default(); BANDS]; MAX_CHANNELS],
            fast: [[0.0; BANDS]; MAX_CHANNELS],
            slow: [[0.0; BANDS]; MAX_CHANNELS],
            fast_coefficient: 0.0,
            slow_coefficient: 0.0,
            rate: 48_000,
        }
    }

    fn refresh(&mut self) {
        self.fast_coefficient = math::one_pole_alpha(5.0, self.rate);
        self.slow_coefficient = math::one_pole_alpha(250.0, self.rate);
        for channel in 0..MAX_CHANNELS {
            for (band, split) in SPLITS.iter().enumerate() {
                let coefficients = Coefficients::design(
                    FilterKind::LowPass,
                    self.rate as f32,
                    *split,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                );
                self.filters[channel][band].set_coefficients(coefficients);
            }
        }
    }

    fn band_levels(&self) -> ([f32; BANDS], [bool; BANDS], [bool; BANDS]) {
        let mut intensity = [0.0f32; BANDS];
        let mut mute = [false; BANDS];
        let mut bypass = [false; BANDS];
        for band in 0..BANDS {
            intensity[band] = self.bands[band][INTENSITY].value().clamp(-40.0, 32.0);
            mute[band] = self.bands[band][MUTE].flag();
            bypass[band] = self.bands[band][BYPASS].flag();
        }
        (intensity, mute, bypass)
    }
}

impl Effect for Crystalizer {
    fn id(&self) -> &'static str {
        "crystalizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.refresh();
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.filters {
            for biquad in channel {
                biquad.reset();
            }
        }
        self.fast = [[0.0; BANDS]; MAX_CHANNELS];
        self.slow = [[0.0; BANDS]; MAX_CHANNELS];
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let (intensity, mute, bypass) = self.band_levels();
        let output_gain = math::db_to_gain(self.gain.value());
        let channels = buffer.channels().min(MAX_CHANNELS);
        let fast_coefficient = self.fast_coefficient;
        let slow_coefficient = self.slow_coefficient;

        for channel in 0..channels {
            let filters = &mut self.filters[channel];
            let fast = &mut self.fast[channel];
            let slow = &mut self.slow[channel];
            for sample in buffer.channel_mut(channel) {
                let input = *sample;
                let mut low = [0.0f32; BANDS];
                for band in 0..BANDS {
                    low[band] = filters[band].process(input);
                }
                let mut sum = 0.0f32;
                for band in 0..BANDS {
                    let value = if band == 0 {
                        low[0]
                    } else if band + 1 == BANDS {
                        input - low[BANDS - 2]
                    } else {
                        low[band] - low[band - 1]
                    };
                    if mute[band] {
                        continue;
                    }
                    if bypass[band] || intensity[band] == 0.0 {
                        sum += value;
                        continue;
                    }
                    let rectified = value.abs();
                    fast[band] += (rectified - fast[band]) * fast_coefficient;
                    slow[band] += (rectified - slow[band]) * slow_coefficient;
                    let transient =
                        ((fast[band] - slow[band]) / (fast[band] + 1.0e-6)).clamp(0.0, 1.0);
                    sum += value * math::db_to_gain(intensity[band] * transient);
                }
                *sample = math::sanitize(sum * output_gain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f32, amplitude: f32, frames: usize, offset: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                let phase = std::f32::consts::TAU * frequency * (offset + index) as f32 / 48_000.0;
                *sample = amplitude * phase.sin();
            }
        }
        buffer
    }

    fn impulse(frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        buffer.channel_mut(0)[0] = 1.0;
        buffer.channel_mut(1)[0] = 1.0;
        buffer
    }

    fn prepared(settings: &[(&str, f32)]) -> Crystalizer {
        let params = ParamSet::new(specs());
        for (id, value) in settings {
            params.set_f32(id, *value).unwrap();
        }
        let mut effect = Crystalizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        effect
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

    #[test]
    fn there_are_thirteen_bands() {
        let specs = specs();
        assert_eq!(specs.len(), 1 + BANDS * 3);
        for ids in &BAND_IDS {
            for id in ids {
                assert!(specs.iter().any(|spec| spec.id == *id), "{id}");
            }
        }
    }

    #[test]
    fn transients_are_emphasised() {
        let mut effect = prepared(&[("band7_intensity", 32.0)]);
        let mut buffer = impulse(512);
        effect.process(&mut buffer);
        assert!(buffer.peak() > 1.0, "peak {}", buffer.peak());
    }

    #[test]
    fn muting_a_band_removes_it() {
        let mut muted = prepared(&[("band1_mute", 1.0)]);
        let mut opened = prepared(&[]);
        let mut a = tone(48.0, 0.5, 8_192, 0);
        let mut b = tone(48.0, 0.5, 8_192, 0);
        for _ in 0..4 {
            muted.process(&mut a);
            opened.process(&mut b);
        }
        assert!(
            energy(&a) < energy(&b) * 0.2,
            "{} {}",
            energy(&a),
            energy(&b)
        );
        assert!(energy(&a).is_finite());
    }

    #[test]
    fn a_bypassed_band_is_not_emphasised() {
        let mut effect = prepared(&[("band7_intensity", 32.0), ("band7_bypass", 1.0)]);
        let mut buffer = impulse(512);
        effect.process(&mut buffer);
        assert!(buffer.peak() <= 1.0 + 1.0e-6, "peak {}", buffer.peak());
    }

    #[test]
    fn a_steady_tone_is_barely_changed() {
        let mut effect = prepared(&[("band5_intensity", 32.0)]);
        for block in 0..64 {
            let mut buffer = tone(700.0, 0.5, 1_024, block * 1_024);
            effect.process(&mut buffer);
        }
        let mut buffer = tone(700.0, 0.5, 1_024, 64 * 1_024);
        effect.process(&mut buffer);
        assert!(buffer.peak() < 0.75, "peak {}", buffer.peak());
    }

    #[test]
    fn the_output_stays_finite_under_abuse() {
        let mut settings: Vec<(&str, f32)> = vec![("gain", 12.0)];
        for ids in &BAND_IDS {
            settings.push((ids[INTENSITY], 32.0));
        }
        let mut effect = prepared(&settings);
        for block in 0..64 {
            let mut buffer = tone(3_000.0, 1.0, 1_024, block * 1_024);
            buffer.channel_mut(0)[0] = 8.0;
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 4_096.0, "{sample}");
                }
            }
        }
    }
}
