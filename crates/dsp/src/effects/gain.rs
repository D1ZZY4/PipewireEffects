use pipewire_effects_core::{
    AudioBlock, AudioEffect, AudioFormat, EffectCategory, EffectDef, EffectKind, ParamSet,
    ParamSpec, ParamUnit,
};

use crate::util::{Ramp, db_to_linear};

const PARAM_GAIN: usize = 0;
const PARAM_MUTE: usize = 1;

pub const KIND: EffectKind = EffectKind::new("gain");

pub const DEF: EffectDef = EffectDef::new(
    KIND,
    "Output Gain",
    "Applies a constant gain to every channel, with an optional mute.",
    EffectCategory::Utility,
    GainEffect::boxed,
);

#[derive(Debug)]
pub struct GainEffect {
    parameters: ParamSet,
    ramp: Ramp,
}

impl GainEffect {
    #[must_use]
    pub fn new() -> Self {
        Self {
            parameters: ParamSet::new(vec![
                ParamSpec::float("gain", "Gain", -60.0, 24.0, 0.0, 0.1, ParamUnit::Decibel)
                    .with_doc("Gain applied to every channel, in decibels."),
                ParamSpec::boolean("mute", "Mute", false)
                    .with_doc("Silences the output without losing the gain setting."),
            ]),
            ramp: Ramp::new(1.0),
        }
    }

    #[must_use]
    pub fn boxed() -> Box<dyn AudioEffect> {
        Box::new(Self::new())
    }

    fn target_gain(&self) -> f32 {
        if self.parameters.value_bool(PARAM_MUTE) {
            0.0
        } else {
            db_to_linear(self.parameters.value_f32(PARAM_GAIN))
        }
    }
}

impl Default for GainEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEffect for GainEffect {
    fn kind(&self) -> EffectKind {
        KIND
    }

    fn parameters(&self) -> &ParamSet {
        &self.parameters
    }

    fn prepare(&mut self, _format: AudioFormat) {
        self.ramp.reset(self.target_gain());
    }

    fn reset(&mut self) {
        self.ramp.reset(self.target_gain());
    }

    fn process(&mut self, block: &mut AudioBlock<'_>) {
        self.ramp.set_target(self.target_gain());
        self.ramp.begin(block.frames());

        for frame in block.frames_iter_mut() {
            let gain = self.ramp.next_value();
            for sample in frame.iter_mut() {
                *sample *= gain;
            }
        }

        self.ramp.finish();
    }

    fn summary(&self) -> String {
        if self.parameters.value_bool(PARAM_MUTE) {
            return String::from("Muted");
        }
        self.parameters.format(PARAM_GAIN)
    }
}
