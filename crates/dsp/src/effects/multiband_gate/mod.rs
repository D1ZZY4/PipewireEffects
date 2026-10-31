use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::crossover::{Crossover, DEFAULT_BANDS, MAX_BANDS};
use crate::effects::dynamics::ballistics;
use crate::effects::multiband;
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const ENABLED: usize = 0;
const THRESHOLD: usize = 1;
const RANGE: usize = 2;
const ATTACK: usize = 3;
const HOLD: usize = 4;
const RELEASE: usize = 5;
const MAKEUP: usize = 6;
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
    [
        "band5_enabled",
        "band5_threshold",
        "band5_range",
        "band5_attack",
        "band5_hold",
        "band5_release",
        "band5_makeup",
    ],
    [
        "band6_enabled",
        "band6_threshold",
        "band6_range",
        "band6_attack",
        "band6_hold",
        "band6_release",
        "band6_makeup",
    ],
    [
        "band7_enabled",
        "band7_threshold",
        "band7_range",
        "band7_attack",
        "band7_hold",
        "band7_release",
        "band7_makeup",
    ],
    [
        "band8_enabled",
        "band8_threshold",
        "band8_range",
        "band8_attack",
        "band8_hold",
        "band8_release",
        "band8_makeup",
    ],
];

const SOLO_IDS: [&str; MAX_BANDS] = [
    "band1_solo",
    "band2_solo",
    "band3_solo",
    "band4_solo",
    "band5_solo",
    "band6_solo",
    "band7_solo",
    "band8_solo",
];

const MUTE_IDS: [&str; MAX_BANDS] = [
    "band1_mute",
    "band2_mute",
    "band3_mute",
    "band4_mute",
    "band5_mute",
    "band6_mute",
    "band7_mute",
    "band8_mute",
];

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = multiband::crossover_specs();
    specs.reserve(1 + MAX_BANDS * 9);
    for (band, ids) in BAND_IDS.iter().enumerate() {
        let group = multiband::band_group(band);
        specs.push(ParamSpec::boolean(ids[ENABLED], "Enabled", true).group(group));
        specs.push(ParamSpec::boolean(SOLO_IDS[band], "Solo", false).group(group));
        specs.push(ParamSpec::boolean(MUTE_IDS[band], "Mute", false).group(group));
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
    specs.push(multiband::mix_spec());
    specs
}

pub struct MultibandGate {
    crossover: Crossover,
    bands: [[Knob; 7]; MAX_BANDS],
    solo: [Knob; MAX_BANDS],
    mute: [Knob; MAX_BANDS],
    count: Knob,
    splits: [Knob; MAX_BANDS - 1],
    mix: Knob,
    envelope: [f32; MAX_BANDS],
    hold_frames: [u32; MAX_BANDS],
    signature: [f32; MAX_BANDS],
    rate: f32,
    dirty: bool,
}

impl MultibandGate {
    pub fn new(params: ParamSet) -> Self {
        let bands = multiband::band_knobs(&params, &BAND_IDS, &DEFAULTS);
        Self {
            crossover: Crossover::default(),
            bands,
            solo: std::array::from_fn(|band| Knob::new(&params, SOLO_IDS[band], 0.0)),
            mute: std::array::from_fn(|band| Knob::new(&params, MUTE_IDS[band], 0.0)),
            count: Knob::new(&params, "bands", DEFAULT_BANDS as f32),
            splits: multiband::split_knobs(&params),
            mix: Knob::new(&params, "mix", 100.0),
            envelope: [1.0; MAX_BANDS],
            hold_frames: [0; MAX_BANDS],
            signature: [f32::NAN; MAX_BANDS],
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
        let count = multiband::band_count(&self.count);
        let splits = multiband::splits(&self.splits);
        multiband::redesign(
            &mut self.crossover,
            &mut self.signature,
            &mut self.dirty,
            count,
            splits,
            self.rate,
        );
        let mut attack = [0.0f32; MAX_BANDS];
        let mut release = [0.0f32; MAX_BANDS];
        let mut threshold = [0.0f32; MAX_BANDS];
        let mut closed = [0.0f32; MAX_BANDS];
        let mut hold = [0u32; MAX_BANDS];
        let mut makeup = [0.0f32; MAX_BANDS];
        let mut enabled = [false; MAX_BANDS];
        let mut solo = [false; MAX_BANDS];
        let mut mute = [false; MAX_BANDS];
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
            solo[band] = self.solo[band].flag();
            mute[band] = self.mute[band].flag();
        }
        let any_solo = solo.iter().take(count).any(|value| *value);
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
                let plays = if any_solo { solo[band] } else { !mute[band] };
                let gain = if plays {
                    self.envelope[band] * math::db_to_gain(makeup[band])
                } else {
                    0.0
                };
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
#[path = "tests/tests.rs"]
mod tests;
