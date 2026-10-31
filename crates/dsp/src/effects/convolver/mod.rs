use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::knob::Knob;

use impulse::RESPONSES;
use partitioned::{BLOCK, Partitioned};

mod impulse;
mod partitioned;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("response", "Response", RESPONSES, 0).group("Impulse"),
        ParamSpec::boolean("auto_gain", "Auto gain", true).group("Impulse"),
        ParamSpec::float("wet", "Wet", ParamUnit::Decibel)
            .range(-60.0, 0.0, 0.0)
            .step(0.5)
            .group("Output")
            .build(),
        ParamSpec::float("dry", "Dry", ParamUnit::Decibel)
            .range(-60.0, 0.0, -60.0)
            .step(0.5)
            .group("Output")
            .build(),
        ParamSpec::float("width", "Width", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-30.0, 30.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    ]
}

pub struct Convolver {
    response: Knob,
    auto_gain: Knob,
    wet: Knob,
    dry: Knob,
    width: Knob,
    output_gain: Knob,
    filters: Vec<[Partitioned; 2]>,
    fallback_scale: Vec<[f32; 2]>,
    rate: u32,
}

impl Convolver {
    pub fn new(params: ParamSet) -> Self {
        let mut convolver = Self {
            response: Knob::new(&params, "response", 0.0),
            auto_gain: Knob::new(&params, "auto_gain", 1.0),
            wet: Knob::new(&params, "wet", 0.0),
            dry: Knob::new(&params, "dry", -60.0),
            width: Knob::new(&params, "width", 100.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            filters: Vec::new(),
            fallback_scale: Vec::new(),
            rate: 48_000,
        };
        convolver.rebuild(48_000);
        convolver
    }

    fn rebuild(&mut self, rate: u32) {
        self.rate = rate.max(8_000);
        self.filters.clear();
        self.fallback_scale.clear();
        for choice in 0..RESPONSES.len() {
            let left = impulse::normalised(choice, self.rate, 0, true);
            let right = impulse::normalised(choice, self.rate, 1, true);
            let left_peak = peak(&left);
            let right_peak = peak(&right);
            self.fallback_scale
                .push([0.5 / left_peak.max(1.0e-9), 0.5 / right_peak.max(1.0e-9)]);
            self.filters.push([
                Partitioned::new(BLOCK, &left),
                Partitioned::new(BLOCK, &right),
            ]);
        }
    }
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0f32, |current, value| current.max(value.abs()))
}

impl Effect for Convolver {
    fn id(&self) -> &'static str {
        "convolver"
    }

    fn prepare(&mut self, format: AudioFormat) {
        if format.rate() != self.rate {
            self.rebuild(format.rate());
        } else {
            self.reset();
        }
    }

    fn reset(&mut self) {
        for choice in &mut self.filters {
            for filter in choice {
                filter.reset();
            }
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let choice = self
            .response
            .index()
            .min(self.filters.len().saturating_sub(1));
        let auto_gain = self.auto_gain.flag();
        let channels = buffer.channels().min(2);
        let width = if channels < 2 {
            1.0
        } else {
            (self.width.value() / 100.0).clamp(0.0, 1.0)
        };
        let wet = math::db_to_gain(self.wet.value());
        let dry = math::db_to_gain(self.dry.value());
        let output = math::db_to_gain(self.output_gain.value());
        let scale = if auto_gain {
            [1.0f32; 2]
        } else {
            self.fallback_scale[choice]
        };
        let frames = buffer.frames();
        for frame in 0..frames {
            let mut convolved = [0.0f32; 2];
            for channel in 0..channels {
                let input = buffer.channel(channel)[frame];
                convolved[channel] = self.filters[choice][channel].feed(input) * scale[channel];
            }
            for channel in 0..channels {
                let input = buffer.channel(channel)[frame];
                let other = convolved[1 - channel];
                let mixed = convolved[channel] * width + other * (1.0 - width);
                buffer.channel_mut(channel)[frame] =
                    math::sanitize((input * dry + mixed * wet) * output);
            }
        }
    }

    fn latency_frames(&self) -> usize {
        BLOCK
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
