use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::crossover::{Crossover, MAX_BANDS};
use crate::effects::dynamics::ballistics;
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const ENABLED: usize = 0;
const THRESHOLD: usize = 1;
const RANGE: usize = 2;
const ATTACK: usize = 3;
const HOLD: usize = 4;
const RELEASE: usize = 5;
const MAKEUP: usize = 6;
const DEFAULT_SPLITS: [f32; MAX_BANDS - 1] = [120.0, 800.0, 5_000.0];
const DEFAULTS: [f32; 7] = [1.0, -40.0, 40.0, 5.0, 50.0, 300.0, 0.0];

const BAND_IDS: [[&str; 7]; MAX_BANDS] = [
    [
        "band1_enabled",
        "band1_threshold",
        "band1_range",
        "band1_attack",
        "band1_hold",
        "band1_release",
        "band1_makeup",
    ],
    [
        "band2_enabled",
        "band2_threshold",
        "band2_range",
        "band2_attack",
        "band2_hold",
        "band2_release",
        "band2_makeup",
    ],
    [
        "band3_enabled",
        "band3_threshold",
        "band3_range",
        "band3_attack",
        "band3_hold",
        "band3_release",
        "band3_makeup",
    ],
    [
        "band4_enabled",
        "band4_threshold",
        "band4_range",
        "band4_attack",
        "band4_hold",
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
                .range(-80.0, 0.0, DEFAULTS[THRESHOLD])
                .step(0.5)
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[RANGE], "Range", ParamUnit::Decibel)
                .range(0.0, 90.0, DEFAULTS[RANGE])
                .step(1.0)
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[ATTACK], "Attack", ParamUnit::Milliseconds)
                .range(0.1, 100.0, DEFAULTS[ATTACK])
                .logarithmic()
                .group(group)
                .build(),
        );
        specs.push(
            ParamSpec::float(ids[HOLD], "Hold", ParamUnit::Milliseconds)
                .range(0.0, 2_000.0, DEFAULTS[HOLD])
                .step(5.0)
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

pub struct MultibandGate {
    crossover: Crossover,
    bands: [[Knob; 7]; MAX_BANDS],
    count: Knob,
    split1: Knob,
    split2: Knob,
    split3: Knob,
    mix: Knob,
    envelope: [f32; MAX_BANDS],
    hold_frames: [u32; MAX_BANDS],
    signature: [f32; 4],
    rate: f32,
    dirty: bool,
}

impl MultibandGate {
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
            hold_frames: [0; MAX_BANDS],
            signature: [f32::NAN; 4],
            rate: 48_000.0,
            dirty: true,
        }
    }
}

impl Effect for MultibandGate {
    fn id(&self) -> &'static str {
        "multiband_gate"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        self.envelope = [1.0; MAX_BANDS];
        self.hold_frames = [0; MAX_BANDS];
        self.crossover.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let count = (self.count.value().round() as usize).clamp(2, MAX_BANDS);
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
        let mut closed = [0.0f32; MAX_BANDS];
        let mut hold = [0u32; MAX_BANDS];
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
            closed[band] = math::db_to_gain(-self.bands[band][RANGE].value());
            hold[band] = (self.bands[band][HOLD].value() / 1_000.0 * self.rate).max(0.0) as u32;
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
                let open = math::gain_to_db(peak) >= threshold[band];
                if open {
                    self.hold_frames[band] = hold[band];
                } else if self.hold_frames[band] > 0 {
                    self.hold_frames[band] -= 1;
                }
                let target = if !enabled[band] || open || self.hold_frames[band] > 0 {
                    1.0
                } else {
                    closed[band]
                };
                let coefficient = if target < self.envelope[band] {
                    release[band]
                } else {
                    attack[band]
                };
                self.envelope[band] += (target - self.envelope[band]) * coefficient;
                if !self.envelope[band].is_finite() {
                    self.envelope[band] = 0.0;
                }
                let gain = self.envelope[band] * math::db_to_gain(makeup[band]);
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

    fn peak_after(effect: &mut MultibandGate, amplitude: f32, blocks: usize) -> f32 {
        let frames = 1024;
        let mut peak = 0.0f32;
        for block in 0..blocks * 2 {
            let mut buffer = tone(1_000.0, amplitude, frames, block * frames);
            effect.process(&mut buffer);
            if block + blocks >= blocks * 2 {
                peak = peak.max(buffer.peak());
            }
        }
        peak
    }

    #[test]
    fn a_loud_signal_stays_open() {
        let params = ParamSet::new(specs());
        for ids in BAND_IDS {
            params.set_f32(ids[THRESHOLD], -40.0).unwrap();
        }
        let mut effect = MultibandGate::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let peak = peak_after(&mut effect, 0.5, 32);
        assert!((peak - 0.5).abs() < 0.05, "{peak}");
    }

    #[test]
    fn a_quiet_signal_is_closed_by_the_range() {
        let params = ParamSet::new(specs());
        for ids in BAND_IDS {
            params.set_f32(ids[THRESHOLD], -20.0).unwrap();
            params.set_f32(ids[RANGE], 60.0).unwrap();
            params.set_f32(ids[HOLD], 0.0).unwrap();
            params.set_f32(ids[RELEASE], 10.0).unwrap();
        }
        let mut effect = MultibandGate::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let peak = peak_after(&mut effect, 0.001, 64);
        let expected = 0.001 * math::db_to_gain(-60.0);
        assert!((peak - expected).abs() < 1.0e-5, "{peak} {expected}");
    }

    #[test]
    fn a_disabled_band_is_not_gated() {
        let params = ParamSet::new(specs());
        for ids in BAND_IDS {
            params.set_f32(ids[ENABLED], 0.0).unwrap();
            params.set_f32(ids[THRESHOLD], -20.0).unwrap();
            params.set_f32(ids[RANGE], 60.0).unwrap();
        }
        let mut effect = MultibandGate::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let peak = peak_after(&mut effect, 0.001, 64);
        assert!((peak - 0.001).abs() < 1.0e-4, "{peak}");
    }
}
