use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

mod parts;

use parts::Resampler2x;

const BANDS: usize = 13;
const MAX_STAGES: usize = 4;
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

type Levels = ([f32; BANDS], [bool; BANDS], [bool; BANDS]);

fn group_for(band: usize) -> &'static str {
    match band {
        0..=3 => mark("Low"),
        4..=7 => mark("Mid"),
        _ => mark("High"),
    }
}

fn stage_count(transition: f32) -> usize {
    ((120.0 / transition.max(1.0)).round() as usize).clamp(1, MAX_STAGES)
}

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(4 + BANDS * 3);
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
        ParamSpec::boolean("adaptive_intensity", "Adaptive intensity", false).group("Adaptive"),
    );
    specs.push(ParamSpec::boolean("oversampling", "Oversampling", false).group("Adaptive"));
    specs.push(
        ParamSpec::float("transition_band", "Transition band", ParamUnit::Hertz)
            .range(10.0, 1_000.0, 120.0)
            .logarithmic()
            .group("Adaptive")
            .build(),
    );
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
    adaptive: Knob,
    oversampling: Knob,
    transition: Knob,
    bands: [[Knob; 3]; BANDS],
    filters: [[[Biquad; MAX_STAGES]; BANDS]; MAX_CHANNELS],
    resamplers: [Resampler2x; MAX_CHANNELS],
    fast: [[f32; BANDS]; MAX_CHANNELS],
    slow: [[f32; BANDS]; MAX_CHANNELS],
    fast_coefficient: f32,
    slow_coefficient: f32,
    rate: u32,
    stages: usize,
    factor: u32,
    signature: (u32, usize, u32),
}

impl Crystalizer {
    pub fn new(params: ParamSet) -> Self {
        Self {
            gain: Knob::new(&params, "gain", 0.0),
            adaptive: Knob::new(&params, "adaptive_intensity", 0.0),
            oversampling: Knob::new(&params, "oversampling", 0.0),
            transition: Knob::new(&params, "transition_band", 120.0),
            bands: std::array::from_fn(|band| {
                std::array::from_fn(|param| Knob::new(&params, BAND_IDS[band][param], 0.0))
            }),
            filters: [[[Biquad::default(); MAX_STAGES]; BANDS]; MAX_CHANNELS],
            resamplers: std::array::from_fn(|_| Resampler2x::new()),
            fast: [[0.0; BANDS]; MAX_CHANNELS],
            slow: [[0.0; BANDS]; MAX_CHANNELS],
            fast_coefficient: 0.0,
            slow_coefficient: 0.0,
            rate: 48_000,
            stages: 1,
            factor: 1,
            signature: (0, 0, 0),
        }
    }

    fn configure(&mut self) {
        let stages = stage_count(self.transition.value());
        let factor = if self.oversampling.flag() { 2 } else { 1 };
        let signature = (self.rate, stages, factor);
        if signature == self.signature {
            return;
        }
        self.signature = signature;
        self.stages = stages;
        self.factor = factor;
        let design_rate = (self.rate.max(1) * factor) as f32;
        self.fast_coefficient = math::one_pole_alpha(5.0, self.rate.max(1) * factor);
        self.slow_coefficient = math::one_pole_alpha(250.0, self.rate.max(1) * factor);
        for channel in 0..MAX_CHANNELS {
            for (band, split) in SPLITS.iter().enumerate() {
                let coefficients = Coefficients::design(
                    FilterKind::LowPass,
                    design_rate,
                    *split,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                );
                for stage in &mut self.filters[channel][band] {
                    stage.set_coefficients(coefficients);
                }
            }
        }
        self.reset();
    }

    fn band_levels(&self) -> Levels {
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

    fn enhance(&mut self, channel: usize, input: f32, levels: &Levels, adaptive: bool) -> f32 {
        let (intensity, mute, bypass) = levels;
        let filters = &mut self.filters[channel];
        let fast = &mut self.fast[channel];
        let slow = &mut self.slow[channel];
        let stages = self.stages;
        let mut low = [0.0f32; BANDS];
        for band in 0..BANDS {
            let mut value = input;
            for stage in filters[band].iter_mut().take(stages) {
                value = stage.process(value);
            }
            low[band] = value;
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
            fast[band] += (rectified - fast[band]) * self.fast_coefficient;
            slow[band] += (rectified - slow[band]) * self.slow_coefficient;
            let transient = ((fast[band] - slow[band]) / (fast[band] + 1.0e-6)).clamp(0.0, 1.0);
            let mut decibels = intensity[band] * transient;
            if adaptive {
                let ratio = (fast[band] / (slow[band] + 1.0e-6)).clamp(0.5, 2.0);
                decibels *= ratio;
            }
            sum += value * math::db_to_gain(decibels);
        }
        sum
    }
}

impl Effect for Crystalizer {
    fn id(&self) -> &'static str {
        "crystalizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.signature = (0, 0, 0);
        self.configure();
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.filters {
            for band in channel {
                for stage in band {
                    stage.reset();
                }
            }
        }
        for resampler in &mut self.resamplers {
            resampler.reset();
        }
        self.fast = [[0.0; BANDS]; MAX_CHANNELS];
        self.slow = [[0.0; BANDS]; MAX_CHANNELS];
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.configure();
        let levels = self.band_levels();
        let adaptive = self.adaptive.flag();
        let output_gain = math::db_to_gain(self.gain.value());
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();
        let factor = self.factor;

        for channel in 0..channels {
            for index in 0..frames {
                let input = buffer.channel(channel)[index];
                let enhanced = if factor == 2 {
                    let pair = self.resamplers[channel].upsample(input);
                    let first = self.enhance(channel, pair[0], &levels, adaptive);
                    let second = self.enhance(channel, pair[1], &levels, adaptive);
                    self.resamplers[channel].downsample([first, second])
                } else {
                    self.enhance(channel, input, &levels, adaptive)
                };
                buffer.channel_mut(channel)[index] = math::sanitize(enhanced * output_gain);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
