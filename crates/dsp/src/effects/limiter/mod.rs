use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const MAX_LOOKAHEAD_RATE: usize = 192_000;
const MAX_LOOKAHEAD_MS: usize = 20;
const MAX_LOOKAHEAD_FRAMES: usize = MAX_LOOKAHEAD_RATE * MAX_LOOKAHEAD_MS / 1000;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .step(0.1)
            .group("Input")
            .build(),
        ParamSpec::float("limit", "Limit", ParamUnit::Decibel)
            .range(-30.0, 0.0, -1.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("lookahead", "Lookahead", ParamUnit::Milliseconds)
            .range(0.1, MAX_LOOKAHEAD_MS as f32, 5.0)
            .step(0.1)
            .group("Timing")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(5.0, 1000.0, 60.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("stereo_link", "Stereo link", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Timing")
            .build(),
    ]
}

struct Channel {
    delay: Vec<f32>,
    head: usize,
    hold: f32,
    envelope: f32,
    goal: f32,
    slope: f32,
    steps: usize,
}

impl Channel {
    fn new() -> Self {
        Self {
            delay: vec![0.0; MAX_LOOKAHEAD_FRAMES + 1],
            head: 0,
            hold: 1.0,
            envelope: 1.0,
            goal: 1.0,
            slope: 0.0,
            steps: 0,
        }
    }

    fn reset(&mut self) {
        self.delay.fill(0.0);
        self.head = 0;
        self.hold = 1.0;
        self.envelope = 1.0;
        self.goal = 1.0;
        self.slope = 0.0;
        self.steps = 0;
    }

    fn advance(&mut self, target: f32, lookahead: usize, release: f32, exact: f32) {
        if target < self.goal {
            self.goal = target;
            self.slope = (self.envelope - target) / lookahead.max(1) as f32;
            self.steps = lookahead;
        }
        if self.steps > 0 {
            self.steps -= 1;
            self.envelope = (self.envelope - self.slope).max(self.goal);
        } else {
            self.goal = 1.0;
            self.envelope += (1.0 - self.envelope) * release;
        }
        self.envelope = self.envelope.min(exact).max(1.0e-5);
    }
}

pub struct Limiter {
    channels: Vec<Channel>,
    input_gain: Knob,
    limit: Knob,
    lookahead: Knob,
    release: Knob,
    link: Knob,
    frames: usize,
    forget: f32,
    release_coefficient: f32,
    input_smoothed: math::Smoothed,
    rate: u32,
}

impl Limiter {
    pub fn new(params: ParamSet) -> Self {
        Self {
            channels: (0..MAX_CHANNELS).map(|_| Channel::new()).collect(),
            input_gain: Knob::new(&params, "input_gain", 0.0),
            limit: Knob::new(&params, "limit", -1.0),
            lookahead: Knob::new(&params, "lookahead", 5.0),
            release: Knob::new(&params, "release", 60.0),
            link: Knob::new(&params, "stereo_link", 100.0),
            frames: 1,
            forget: 0.2,
            release_coefficient: 0.001,
            input_smoothed: math::Smoothed::new(1.0),
            rate: 48_000,
        }
    }

    fn update_coefficient(&mut self) {
        let seconds = (self.release.value().max(0.5) / 1000.0).max(1.0e-6);
        self.release_coefficient = 1.0 - (-1.0 / (seconds * self.rate.max(1) as f32)).exp();
        self.input_smoothed.set_time_constant(20.0, self.rate);
        let frames = (self.lookahead.value() / 1000.0 * self.rate as f32).round() as usize;
        self.frames = frames.clamp(1, MAX_LOOKAHEAD_FRAMES);
        self.forget = 1.0 - (-1.0 / self.frames as f32).exp();
    }

    fn required(peak: f32, ceiling: f32) -> f32 {
        let peak = peak * peak;
        let ceiling_squared = ceiling * ceiling;
        if peak > ceiling_squared {
            (ceiling_squared / peak).sqrt()
        } else {
            1.0
        }
    }
}

impl Effect for Limiter {
    fn id(&self) -> &'static str {
        "limiter"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.update_coefficient();
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.input_smoothed
            .snap(math::db_to_gain(self.input_gain.value()));
    }

    fn latency_frames(&self) -> usize {
        self.frames
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_coefficient();
        self.input_smoothed
            .set_target(math::db_to_gain(self.input_gain.value()));
        let input_gain = self.input_smoothed.advance();
        let ceiling = math::db_to_gain(self.limit.value()).max(1.0e-5);
        let release = self.release_coefficient;
        let forget = self.forget;
        let link = (self.link.value() / 100.0).clamp(0.0, 1.0);
        let channels = buffer.channels().min(self.channels.len());
        let frames = buffer.frames();
        let lookahead = self.frames;
        let mut delayed = [0.0f32; MAX_CHANNELS];
        let mut need = [1.0f32; MAX_CHANNELS];
        let mut gains = [1.0f32; MAX_CHANNELS];

        for frame in 0..frames {
            for (index, channel) in self.channels.iter_mut().enumerate().take(channels) {
                let length = channel.delay.len();
                let read = (channel.head + length - lookahead) % length;
                let incoming = buffer.channel(index)[frame];
                delayed[index] = channel.delay[read];
                channel.delay[channel.head] = incoming;
                channel.head = (channel.head + 1) % length;
                need[index] = Self::required(incoming.abs() * input_gain, ceiling);
            }

            let shared = need
                .iter()
                .take(channels)
                .fold(1.0f32, |acc, value| acc.min(*value));
            for (value, channel) in need.iter().zip(self.channels.iter_mut()).take(channels) {
                channel.hold = if *value < channel.hold {
                    *value
                } else {
                    channel.hold + (1.0 - channel.hold) * forget
                };
            }
            for ((delayed_value, channel), gain) in delayed
                .iter()
                .zip(self.channels.iter_mut())
                .zip(gains.iter_mut())
                .take(channels)
            {
                let own = channel.hold;
                let target = (shared * link + own * (1.0 - link)).min(own);
                let exact = Self::required(delayed_value.abs() * input_gain, ceiling);
                channel.advance(target, lookahead, release, exact);
                *gain = channel.envelope;
            }
            for (index, gain) in gains.iter().enumerate().take(channels) {
                let plan = buffer.channel_mut(index);
                plan[frame] = math::sanitize(delayed[index] * gain * input_gain);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
