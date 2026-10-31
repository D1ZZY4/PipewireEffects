use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Input")
            .build(),
        ParamSpec::float("cutoff", "Cutoff", ParamUnit::Hertz)
            .range(300.0, 2_000.0, 700.0)
            .logarithmic()
            .group("Crossfeed")
            .build(),
        ParamSpec::float("feed", "Feed", ParamUnit::Decibel)
            .range(1.0, 15.0, 4.5)
            .step(0.1)
            .group("Crossfeed")
            .build(),
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    ]
}

pub struct Crossfeed {
    input_gain: Knob,
    cutoff: Knob,
    feed: Knob,
    output_gain: Knob,
    rate: u32,
    state: [f32; MAX_CHANNELS],
}

impl Crossfeed {
    pub fn new(params: ParamSet) -> Self {
        Self {
            input_gain: Knob::new(&params, "input_gain", 0.0),
            cutoff: Knob::new(&params, "cutoff", 700.0),
            feed: Knob::new(&params, "feed", 4.5),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            rate: 48_000,
            state: [0.0; MAX_CHANNELS],
        }
    }
}

fn coefficient(cutoff: f32, rate: u32) -> f32 {
    let rate = rate.max(1) as f32;
    let cutoff = cutoff.clamp(1.0, (rate * 0.45).max(1.0));
    (1.0 - (-std::f32::consts::TAU * cutoff / rate).exp()).clamp(0.0, 1.0)
}

impl Effect for Crossfeed {
    fn id(&self) -> &'static str {
        "crossfeed"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.reset();
    }

    fn reset(&mut self) {
        self.state = [0.0; MAX_CHANNELS];
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        if channels < 2 {
            return;
        }
        let input_gain = math::db_to_gain(self.input_gain.value());
        let output_gain = math::db_to_gain(self.output_gain.value());
        let alpha = coefficient(self.cutoff.value(), self.rate);
        let gain = 0.5 * math::db_to_gain(self.feed.value());
        let frames = buffer.frames();

        for frame in 0..frames {
            let left = buffer.channel_mut(0)[frame] * input_gain;
            let right = buffer.channel_mut(1)[frame] * input_gain;
            let state = &mut self.state;
            state[0] = math::sanitize(state[0] + (left - state[0]) * alpha);
            state[1] = math::sanitize(state[1] + (right - state[1]) * alpha);
            let difference = state[1] - state[0];
            buffer.channel_mut(0)[frame] = math::sanitize((left + gain * difference) * output_gain);
            buffer.channel_mut(1)[frame] =
                math::sanitize((right - gain * difference) * output_gain);
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
