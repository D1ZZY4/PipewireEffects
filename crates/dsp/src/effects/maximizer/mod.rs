use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const MAX_MAKEUP_DB: f32 = 24.0;
const MAX_LOOKAHEAD_RATE: usize = 192_000;
const MAX_LOOKAHEAD_MS: usize = 20;
const MAX_LOOKAHEAD_FRAMES: usize = MAX_LOOKAHEAD_RATE * MAX_LOOKAHEAD_MS / 1000;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Input")
            .build(),
        ParamSpec::float("threshold", "Threshold", ParamUnit::Decibel)
            .range(-30.0, 0.0, 0.0)
            .step(0.1)
            .group("Maximizer")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(1.0, 100.0, 25.0)
            .logarithmic()
            .group("Maximizer")
            .build(),
        ParamSpec::float("lookahead", "Lookahead", ParamUnit::Milliseconds)
            .range(0.0, MAX_LOOKAHEAD_MS as f32, 0.0)
            .step(0.1)
            .group("Timing")
            .build(),
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    ]
}

pub struct Maximizer {
    input_gain: Knob,
    threshold: Knob,
    release: Knob,
    lookahead: Knob,
    output_gain: Knob,
    delay: Vec<Vec<f32>>,
    head: usize,
    gain: f32,
    goal: f32,
    slope: f32,
    steps: usize,
    reference: f32,
    frames: usize,
    rate: u32,
    release_coefficient: f32,
    reference_coefficient: f32,
    input_smoothed: math::Smoothed,
}

impl Maximizer {
    pub fn new(params: ParamSet) -> Self {
        let mut effect = Self {
            input_gain: Knob::new(&params, "input_gain", 0.0),
            threshold: Knob::new(&params, "threshold", 0.0),
            release: Knob::new(&params, "release", 25.0),
            lookahead: Knob::new(&params, "lookahead", 0.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            delay: (0..MAX_CHANNELS)
                .map(|_| vec![0.0; MAX_LOOKAHEAD_FRAMES + 1])
                .collect(),
            head: 0,
            gain: 1.0,
            goal: 1.0,
            slope: 0.0,
            steps: 0,
            reference: 0.0,
            frames: 0,
            rate: 48_000,
            release_coefficient: 0.0,
            reference_coefficient: 0.0,
            input_smoothed: math::Smoothed::new(1.0),
        };
        effect.update_ballistics();
        effect
    }

    fn update_ballistics(&mut self) {
        self.release_coefficient = math::one_pole_alpha(self.release.value(), self.rate);
        self.reference_coefficient = math::one_pole_alpha(300.0, self.rate);
        self.input_smoothed.set_time_constant(20.0, self.rate);
        let frames = (self.lookahead.value() / 1000.0 * self.rate as f32).round() as usize;
        self.frames = frames.clamp(0, MAX_LOOKAHEAD_FRAMES);
    }
}

impl Effect for Maximizer {
    fn id(&self) -> &'static str {
        "maximizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(1);
        self.update_ballistics();
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.delay {
            channel.fill(0.0);
        }
        self.head = 0;
        self.gain = 1.0;
        self.goal = 1.0;
        self.slope = 0.0;
        self.steps = 0;
        self.reference = 0.0;
        self.input_smoothed
            .snap(math::db_to_gain(self.input_gain.value()));
    }

    fn latency_frames(&self) -> usize {
        self.frames
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_ballistics();
        self.input_smoothed
            .set_target(math::db_to_gain(self.input_gain.value()));
        let input_gain = self.input_smoothed.advance();
        let output_gain = math::db_to_gain(self.output_gain.value());
        let ceiling = math::db_to_gain(self.threshold.value()).clamp(1.0e-3, 1.0);
        let limit = math::db_to_gain(MAX_MAKEUP_DB);
        let release = self.release_coefficient;
        let reference_coefficient = self.reference_coefficient;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();
        let length = MAX_LOOKAHEAD_FRAMES + 1;
        let lookahead = self.frames.min(length - 1);

        for frame in 0..frames {
            let read = (self.head + length - lookahead) % length;
            let mut delayed = [0.0f32; MAX_CHANNELS];
            let mut peak = 0.0f32;
            for (channel, slot) in delayed.iter_mut().enumerate().take(channels) {
                let incoming = buffer.channel(channel)[frame] * input_gain;
                if lookahead == 0 {
                    *slot = incoming;
                } else {
                    let ring = &mut self.delay[channel];
                    *slot = ring[read];
                    ring[self.head] = incoming;
                }
                peak = peak.max(incoming.abs());
            }
            if lookahead > 0 {
                self.head = (self.head + 1) % length;
            }
            self.reference += (peak - self.reference) * reference_coefficient;
            if !self.reference.is_finite() {
                self.reference = 0.0;
            }
            let makeup = (ceiling / self.reference.max(1.0e-6)).clamp(1.0, limit);
            let required = if peak > ceiling { ceiling / peak } else { 1.0 };
            if required < self.goal {
                self.goal = required;
                self.slope = (self.gain - required) / lookahead.max(1) as f32;
                self.steps = lookahead;
            }
            if self.steps > 0 {
                self.steps -= 1;
                self.gain = (self.gain - self.slope).max(self.goal);
            } else {
                self.goal = 1.0;
                self.gain += (1.0 - self.gain) * release;
            }
            self.gain = self.gain.min(required).max(1.0e-5);
            if !self.gain.is_finite() {
                self.gain = 1.0;
            }
            let scale = self.gain * makeup;
            for (channel, value) in delayed.iter().enumerate().take(channels) {
                let sample = (value * scale).clamp(-ceiling, ceiling) * output_gain;
                buffer.channel_mut(channel)[frame] = math::sanitize(sample);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
