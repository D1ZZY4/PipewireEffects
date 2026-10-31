use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util;
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
            .range(-30.0, 30.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::boolean("mute", "Mute", false).group("Output"),
    ]
}

pub struct Gain {
    gain: Knob,
    mute: Knob,
    smoothed: math::Smoothed,
    rate: u32,
}

impl Gain {
    pub fn new(params: ParamSet) -> Self {
        Self {
            gain: Knob::new(&params, "gain", 0.0),
            mute: Knob::new(&params, "mute", 0.0),
            smoothed: math::Smoothed::new(1.0),
            rate: 48_000,
        }
    }

    fn update_coefficient(&mut self) {
        self.smoothed.set_time_constant(20.0, self.rate);
    }
}

impl Effect for Gain {
    fn id(&self) -> &'static str {
        "gain"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.update_coefficient();
        self.smoothed.snap(self.target_gain());
    }

    fn reset(&mut self) {
        self.smoothed.snap(self.target_gain());
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        if let Some(gain) = self.elementwise_gain() {
            util::apply_gain(buffer, gain);
        }
    }

    fn elementwise_gain(&mut self) -> Option<f32> {
        let target = self.target_gain();
        self.smoothed.set_target(target);
        Some(self.smoothed.advance())
    }
}

impl Gain {
    fn target_gain(&self) -> f32 {
        if self.mute.flag() {
            0.0
        } else {
            math::db_to_gain(self.gain.value())
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
