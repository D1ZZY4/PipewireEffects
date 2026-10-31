use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::knob::Knob;

const BUFFER_LEN: usize = 384_000;
const CHANNELS: usize = 2;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("delay_left", "Left delay", ParamUnit::Milliseconds)
            .range(1.0, 2000.0, 200.0)
            .logarithmic()
            .group("Delay")
            .build(),
        ParamSpec::float("delay_right", "Right delay", ParamUnit::Milliseconds)
            .range(1.0, 2000.0, 200.0)
            .logarithmic()
            .group("Delay")
            .build(),
        ParamSpec::float("feedback", "Feedback", ParamUnit::Percent)
            .range(0.0, 95.0, 30.0)
            .step(1.0)
            .group("Feedback")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 25.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::boolean("ping_pong", "Ping-pong", false).group("Feedback"),
        ParamSpec::float("tempo", "Tempo", ParamUnit::None)
            .range(20.0, 300.0, 120.0)
            .step(1.0)
            .group("Sync")
            .build(),
        ParamSpec::choice("sync_left", "Left sync", SYNC_LABELS, 0).group("Sync"),
        ParamSpec::choice("sync_right", "Right sync", SYNC_LABELS, 0).group("Sync"),
    ]
}

const SYNC_LABELS: &[&str] = &[
    mark("Off"),
    mark("1/1"),
    mark("1/2"),
    mark("1/4"),
    mark("1/8"),
    mark("1/16"),
    mark("1/32"),
];

pub struct Delay {
    delay_left: Knob,
    delay_right: Knob,
    feedback: Knob,
    mix: Knob,
    ping_pong: Knob,
    tempo: Knob,
    sync_left: Knob,
    sync_right: Knob,
    buffers: [Vec<f32>; CHANNELS],
    write: usize,
    current: [f32; CHANNELS],
    rate: u32,
}

impl Delay {
    pub fn new(params: ParamSet) -> Self {
        Self {
            delay_left: Knob::new(&params, "delay_left", 200.0),
            delay_right: Knob::new(&params, "delay_right", 200.0),
            feedback: Knob::new(&params, "feedback", 30.0),
            mix: Knob::new(&params, "mix", 25.0),
            ping_pong: Knob::new(&params, "ping_pong", 0.0),
            tempo: Knob::new(&params, "tempo", 120.0),
            sync_left: Knob::new(&params, "sync_left", 0.0),
            sync_right: Knob::new(&params, "sync_right", 0.0),
            buffers: [vec![0.0; BUFFER_LEN], vec![0.0; BUFFER_LEN]],
            write: 0,
            current: [0.0; CHANNELS],
            rate: 48_000,
        }
    }

    fn target_samples(&self) -> [f32; CHANNELS] {
        let rate = self.rate.max(1) as f32;
        let beat = 60.0 / self.tempo.value().clamp(20.0, 300.0);
        let limit = BUFFER_LEN as f32 - 4.0;
        [
            (self.seconds(self.sync_left.index(), self.delay_left.value(), beat) * rate)
                .clamp(1.0, limit),
            (self.seconds(self.sync_right.index(), self.delay_right.value(), beat) * rate)
                .clamp(1.0, limit),
        ]
    }

    fn seconds(&self, sync: usize, milliseconds: f32, beat: f32) -> f32 {
        match sync {
            1 => 4.0 * beat,
            2 => 2.0 * beat,
            3 => beat,
            4 => 0.5 * beat,
            5 => 0.25 * beat,
            6 => 0.125 * beat,
            _ => milliseconds / 1000.0,
        }
    }

    fn read(&self, channel: usize, position: f32) -> f32 {
        let buffer = &self.buffers[channel];
        let length = buffer.len() as f32;
        let base = position.rem_euclid(length);
        let lower = base.floor() as usize % buffer.len();
        let upper = (lower + 1) % buffer.len();
        let fraction = base - base.floor();
        buffer[lower] * (1.0 - fraction) + buffer[upper] * fraction
    }
}

impl Effect for Delay {
    fn id(&self) -> &'static str {
        "delay"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.reset();
    }

    fn reset(&mut self) {
        for buffer in &mut self.buffers {
            buffer.fill(0.0);
        }
        self.write = 0;
        self.current = self.target_samples();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(CHANNELS);
        let frames = buffer.frames();
        let target = self.target_samples();
        let feedback = (self.feedback.value() / 100.0).clamp(0.0, 0.95);
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let ping_pong = self.ping_pong.flag();
        let glide = math::one_pole_alpha(120.0, self.rate.max(1));

        for frame in 0..frames {
            for (current, wanted) in self.current.iter_mut().zip(target.iter()) {
                *current += (*wanted - *current) * glide;
                *current = math::sanitize(*current);
            }
            let write = self.write;
            let mut delayed = [0.0f32; CHANNELS];
            for (channel, slot) in delayed.iter_mut().enumerate() {
                let position = write as f32 - self.current[channel];
                *slot = self.read(channel, position.rem_euclid(BUFFER_LEN as f32));
            }
            for channel in 0..channels {
                let input = buffer.channel_mut(channel)[frame];
                let source = if ping_pong {
                    delayed[1 - channel]
                } else {
                    delayed[channel]
                };
                self.buffers[channel][write] = math::sanitize(input + source * feedback);
                buffer.channel_mut(channel)[frame] =
                    math::sanitize(input * (1.0 - mix) + delayed[channel] * mix);
            }
            self.write = (write + 1) % BUFFER_LEN;
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
