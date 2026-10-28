use pipewire_effects_core::{
    AudioBlock, AudioEffect, AudioFormat, EffectCategory, EffectDef, EffectKind, ParamSet,
    ParamSpec, ParamUnit,
};

use crate::util::{db_to_linear, one_pole_coefficient};

const PARAM_THRESHOLD: usize = 0;
const PARAM_MAKEUP: usize = 1;
const PARAM_RELEASE: usize = 2;
const PARAM_LINKED: usize = 3;

pub const KIND: EffectKind = EffectKind::new("limiter");

pub const DEF: EffectDef = EffectDef::new(
    KIND,
    "Limiter",
    "Brick-wall peak limiter with make-up gain and a linked stereo detector.",
    EffectCategory::Dynamics,
    LimiterEffect::boxed,
);

#[derive(Debug)]
pub struct LimiterEffect {
    parameters: ParamSet,
    envelopes: Vec<f32>,
}

impl LimiterEffect {
    #[must_use]
    pub fn new() -> Self {
        let mut effect = Self {
            parameters: ParamSet::new(vec![
                ParamSpec::float(
                    "threshold",
                    "Ceiling",
                    -30.0,
                    0.0,
                    -1.0,
                    0.1,
                    ParamUnit::Decibel,
                )
                .with_doc("The loudest sample the limiter will let through."),
                ParamSpec::float("makeup", "Make-up", 0.0, 24.0, 0.0, 0.1, ParamUnit::Decibel)
                    .with_doc("Gain applied before limiting, to restore loudness."),
                ParamSpec::float_log(
                    "release",
                    "Release",
                    5.0,
                    2_000.0,
                    150.0,
                    ParamUnit::Milliseconds,
                )
                .with_doc("How quickly the limiter recovers once the signal drops."),
                ParamSpec::boolean("linked", "Link channels", true).with_doc(
                    "Reduce all channels by the same amount, preserving the stereo image.",
                ),
            ]),
            envelopes: Vec::new(),
        };
        effect.prepare(AudioFormat::default());
        effect
    }

    #[must_use]
    pub fn boxed() -> Box<dyn AudioEffect> {
        Box::new(Self::new())
    }

    #[must_use]
    pub fn ceiling(&self) -> f32 {
        db_to_linear(self.parameters.value_f32(PARAM_THRESHOLD))
    }
}

impl Default for LimiterEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEffect for LimiterEffect {
    fn kind(&self) -> EffectKind {
        KIND
    }

    fn parameters(&self) -> &ParamSet {
        &self.parameters
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.envelopes = vec![1.0; format.channels_usize().max(1)];
    }

    fn reset(&mut self) {
        self.envelopes.fill(1.0);
    }

    fn process(&mut self, block: &mut AudioBlock<'_>) {
        let channels = block.channels();
        if channels == 0 || channels != self.envelopes.len() {
            return;
        }

        let ceiling = self.ceiling();
        let makeup = db_to_linear(self.parameters.value_f32(PARAM_MAKEUP));
        let linked = self.parameters.value_bool(PARAM_LINKED);
        let release = one_pole_coefficient(
            self.parameters.value_f32(PARAM_RELEASE),
            block.format().sample_rate_f32(),
        );

        for frame in block.frames_iter_mut() {
            let common_level = if linked {
                frame.iter().fold(0.0_f32, |acc, s| acc.max(s.abs())) * makeup
            } else {
                0.0
            };

            for (channel, sample) in frame.iter_mut().enumerate() {
                let level = if linked {
                    common_level
                } else {
                    sample.abs() * makeup
                };

                let target = if level > ceiling {
                    ceiling / level
                } else {
                    1.0
                };
                let envelope = &mut self.envelopes[channel];

                if target < *envelope {
                    *envelope = target;
                } else {
                    *envelope += (target - *envelope) * release;
                }

                *sample *= makeup * *envelope;
            }
        }
    }

    fn summary(&self) -> String {
        format!(
            "Ceiling {} · Make-up {}",
            self.parameters.format(PARAM_THRESHOLD),
            self.parameters.format(PARAM_MAKEUP)
        )
    }
}
