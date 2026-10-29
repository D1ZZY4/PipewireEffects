use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::crossover::{Crossover, MAX_BANDS};
use crate::effects::dynamics::{ballistics, soft_knee_reduction_db};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const ENABLED: usize = 0;
const THRESHOLD: usize = 1;
const RATIO: usize = 2;
const KNEE: usize = 3;
const ATTACK: usize = 4;
const RELEASE: usize = 5;
const MAKEUP: usize = 6;
const DEFAULT_SPLITS: [f32; MAX_BANDS - 1] = [120.0, 800.0, 5_000.0];
const DEFAULTS: [f32; 7] = [1.0, -18.0, 2.0, 6.0, 20.0, 200.0, 0.0];

const BAND_IDS: [[&str; 7]; MAX_BANDS] = [
    [
        "band1_enabled",
        "band1_threshold",
        "band1_ratio",
        "band1_knee",
        "band1_attack",
        "band1_release",
        "band1_makeup",
    ],
    [
        "band2_enabled",
        "band2_threshold",
        "band2_ratio",
        "band2_knee",
        "band2_attack",
        "band2_release",
        "band2_makeup",
    ],
    [
        "band3_enabled",
        "band3_threshold",
        "band3_ratio",
        "band3_knee",
        "band3_attack",
        "band3_release",
        "band3_makeup",
    ],
    [
        "band4_enabled",
        "band4_threshold",
        "band4_ratio",
        "band4_knee",
        "band4_attack",
        "band4_release",
        "band4_makeup",
    ],
];

const BAND_GROUPS: [&str; MAX_BANDS] = [
    mark("Band 1"),
    mark("Band 2"),
    mark("Band 3"),
    mark("Band 4"),
];

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(5 + MAX_BANDS * 7);
    specs.push(
        ParamSpec::float("bands", "Bands", ParamUnit::None)
            .range(2.0, MAX_BANDS as f32, MAX_BANDS as f32)
            .step(1.0)
            .group("Crossovers")
            .build(),
    );
    specs.push(
        ParamSpec::float("split1", "Split 1", ParamUnit::Hertz)
            .range(40.0, 1_000.0, DEFAULT_SPLITS[0])
            .logarithmic()
            .group("Crossovers")
            .build(),
    );
    specs.push(
        ParamSpec::float("split2", "Split 2", ParamUnit::Hertz)
            .range(200.0, 8_000.0, DEFAULT_SPLITS[1])
            .logarithmic()
            .group("Crossovers")
            .build(),
    );
    specs.push(
        ParamSpec::float("split3", "Split 3", ParamUnit::Hertz)
            .range(1_000.0, 16_000.0, DEFAULT_SPLITS[2])
            .logarithmic()
            .group("Crossovers")
            .build(),
    );
    for band in 0..MAX_BANDS {
        let ids = BAND_IDS[band];
        let group = BAND_GROUPS[band];
        specs.push(ParamSpec::boolean(ids[ENABLED], "Enabled", true).group(group));
        specs.push(
            ParamSpec::float(ids[THRESHOLD], "Threshold", ParamUnit::Decibel)
                .range(-60.0, 0.0, DEFAULTS[THRESHOLD])
                .step(0.5)
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[RATIO], "Ratio", ParamUnit::Ratio)
                .range(1.0, 20.0, DEFAULTS[RATIO])
                .step(0.1)
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[KNEE], "Knee", ParamUnit::Decibel)
                .range(0.0, 24.0, DEFAULTS[KNEE])
                .step(0.5)
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[ATTACK], "Attack", ParamUnit::Milliseconds)
                .range(0.1, 200.0, DEFAULTS[ATTACK])
                .logarithmic()
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[RELEASE], "Release", ParamUnit::Milliseconds)
                .range(5.0, 2_000.0, DEFAULTS[RELEASE])
                .logarithmic()
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[MAKEUP], "Makeup", ParamUnit::Decibel)
                .range(-12.0, 24.0, DEFAULTS[MAKEUP])
                .step(0.1)
                .group(group)
                .build(),
        );
    }
    specs.push(
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    );
    specs
}

pub struct MultibandCompressor {
    crossover: Crossover,
    bands: [[Knob; 7]; MAX_BANDS],
    count: Knob,
    split1: Knob,
    split2: Knob,
    split3: Knob,
    mix: Knob,
    envelope: [f32; MAX_BANDS],
    signature: [f32; 4],
    rate: f32,
    dirty: bool,
}

impl MultibandCompressor {
    pub fn new(params: ParamSet) -> Self {
        let bands = std::array::from_fn(|band| {
            std::array::from_fn(|param| Knob::new(&params, BAND_IDS[band][param], DEFAULTS[param]))
        });
        Self {
            crossover: Crossover::default(),
            bands,
            count: Knob::new(&params, "bands", MAX_BANDS as f32),
            split1: Knob::new(&params, "split1", DEFAULT_SPLITS[0]),
            split2: Knob::new(&params, "split2", DEFAULT_SPLITS[1]),
            split3: Knob::new(&params, "split3", DEFAULT_SPLITS[2]),
            mix: Knob::new(&params, "mix", 100.0),
            envelope: [1.0; MAX_BANDS],
            signature: [f32::NAN; 4],
            rate: 48_000.0,
            dirty: true,
        }
    }

    fn band_count(&self) -> usize {
        (self.count.value().round() as usize).clamp(2, MAX_BANDS)
    }
}

impl Effect for MultibandCompressor {
    fn id(&self) -> &'static str {
        "multiband_compressor"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        self.envelope = [1.0; MAX_BANDS];
        self.crossover.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let count = self.band_count();
        let splits = [
            self.split1.value(),
            self.split2.value(),
            self.split3.value(),
        ];
        let signature = [count as f32, splits[0], splits[1], splits[2]];
        if self.dirty || signature != self.signature {
            self.signature = signature;
            self.dirty = false;
            self.crossover.design(count, self.rate, &splits);
        }
        let mut attack = [0.0f32; MAX_BANDS];
        let mut release = [0.0f32; MAX_BANDS];
        let mut threshold = [0.0f32; MAX_BANDS];
        let mut ratio = [0.0f32; MAX_BANDS];
        let mut knee = [0.0f32; MAX_BANDS];
        let mut makeup = [0.0f32; MAX_BANDS];
        let mut enabled = [false; MAX_BANDS];
        for band in 0..count {
            let (a, r) = ballistics(
                self.bands[band][ATTACK].value(),
                self.bands[band][RELEASE].value(),
                self.rate as u32,
            );
            attack[band] = a;
            release[band] = r;
            threshold[band] = self.bands[band][THRESHOLD].value();
            ratio[band] = self.bands[band][RATIO].value().max(1.0);
            knee[band] = self.bands[band][KNEE].value().max(0.0);
            makeup[band] = self.bands[band][MAKEUP].value();
            enabled[band] = self.bands[band][ENABLED].flag();
        }
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();
        for frame in 0..frames {
            let mut accumulated = [0.0f32; MAX_CHANNELS];
            for band in 0..count {
                let mut values = [0.0f32; MAX_CHANNELS];
                let mut peak = 0.0f32;
                for (channel, slot) in values.iter_mut().enumerate().take(channels) {
                    let value = self
                        .crossover
                        .run(channel, band, buffer.channel(channel)[frame]);
                    *slot = value;
                    peak = peak.max(value.abs());
                }
                let target = if enabled[band] {
                    let reduction = soft_knee_reduction_db(
                        math::gain_to_db(peak),
                        threshold[band],
                        knee[band],
                        ratio[band],
                    );
                    math::db_to_gain(reduction + makeup[band])
                } else {
                    1.0
                };
                let coefficient = if target < self.envelope[band] {
                    attack[band]
                } else {
                    release[band]
                };
                self.envelope[band] += (target - self.envelope[band]) * coefficient;
                if !self.envelope[band].is_finite() {
                    self.envelope[band] = 1.0;
                }
                let gain = self.envelope[band];
                for (slot, value) in accumulated.iter_mut().zip(values.iter()).take(channels) {
                    *slot += value * gain;
                }
            }
            for (channel, slot) in accumulated.iter().enumerate().take(channels) {
                let dry = buffer.channel(channel)[frame];
                buffer.channel_mut(channel)[frame] = math::sanitize(dry * (1.0 - mix) + slot * mix);
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
                *sample = amplitude
                    * (std::f32::consts::TAU * frequency * (offset + index) as f32 / 48_000.0)
                        .sin();
            }
        }
        buffer
    }

    fn settled(effect: &mut MultibandCompressor, frequency: f32, amplitude: f32) -> f32 {
        let frames = 1024;
        let blocks = 64;
        let mut peak = 0.0f32;
        for block in 0..blocks {
            let mut buffer = tone(frequency, amplitude, frames, block * frames);
            effect.process(&mut buffer);
            if block + 16 >= blocks {
                peak = peak.max(buffer.peak());
            }
        }
        peak
    }

    #[test]
    fn a_unity_ratio_is_transparent() {
        let params = ParamSet::new(specs());
        for ids in BAND_IDS {
            params.set_f32(ids[RATIO], 1.0).unwrap();
        }
        let mut effect = MultibandCompressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let peak = settled(&mut effect, 1_000.0, 0.5);
        assert!((peak - 0.5).abs() < 0.05, "{peak}");
    }

    #[test]
    fn loud_material_is_reduced() {
        let params = ParamSet::new(specs());
        params.set_f32("bands", 2.0).unwrap();
        for ids in BAND_IDS.iter().take(2) {
            params.set_f32(ids[THRESHOLD], -20.0).unwrap();
            params.set_f32(ids[RATIO], 8.0).unwrap();
        }
        let mut effect = MultibandCompressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let peak = settled(&mut effect, 1_000.0, 0.5);
        assert!(peak < 0.35, "the compressor did not act: {peak}");
    }

    #[test]
    fn makeup_lifts_the_compressed_band() {
        let measure = |makeup: f32| {
            let params = ParamSet::new(specs());
            params.set_f32("bands", 2.0).unwrap();
            for ids in BAND_IDS.iter().take(2) {
                params.set_f32(ids[THRESHOLD], -20.0).unwrap();
                params.set_f32(ids[RATIO], 8.0).unwrap();
                params.set_f32(ids[MAKEUP], makeup).unwrap();
            }
            let mut effect = MultibandCompressor::new(params);
            effect.prepare(AudioFormat::new(48_000, 2));
            settled(&mut effect, 1_000.0, 0.5)
        };
        let without = measure(0.0);
        let with = measure(6.0);
        assert!(with > without * 1.8, "{with} against {without}");
    }

    #[test]
    fn a_disabled_band_passes_its_range_through() {
        let params = ParamSet::new(specs());
        params.set_f32("bands", 2.0).unwrap();
        for ids in BAND_IDS.iter().take(2) {
            params.set_f32(ids[ENABLED], 0.0).unwrap();
        }
        let mut effect = MultibandCompressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let peak = settled(&mut effect, 200.0, 0.5);
        assert!((peak - 0.5).abs() < 0.05, "{peak}");
    }
}
