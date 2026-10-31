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
        ParamSpec::float("dry_left", "Left dry", ParamUnit::Decibel)
            .range(-60.0, 6.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("wet_left", "Left wet", ParamUnit::Decibel)
            .range(-60.0, 6.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("dry_right", "Right dry", ParamUnit::Decibel)
            .range(-60.0, 6.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("wet_right", "Right wet", ParamUnit::Decibel)
            .range(-60.0, 6.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("tape", "Tape", ParamUnit::Percent)
            .range(0.0, 100.0, 0.0)
            .step(1.0)
            .group("Tape")
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
    dry_left: Knob,
    wet_left: Knob,
    dry_right: Knob,
    wet_right: Knob,
    tape: Knob,
    ping_pong: Knob,
    tempo: Knob,
    sync_left: Knob,
    sync_right: Knob,
    buffers: [Vec<f32>; CHANNELS],
    write: usize,
    current: [f32; CHANNELS],
    wow_phase: f32,
    flutter_phase: f32,
    rate: u32,
}

impl Delay {
    pub fn new(params: ParamSet) -> Self {
        Self {
            delay_left: Knob::new(&params, "delay_left", 200.0),
            delay_right: Knob::new(&params, "delay_right", 200.0),
            feedback: Knob::new(&params, "feedback", 30.0),
            mix: Knob::new(&params, "mix", 25.0),
            dry_left: Knob::new(&params, "dry_left", 0.0),
            wet_left: Knob::new(&params, "wet_left", 0.0),
            dry_right: Knob::new(&params, "dry_right", 0.0),
            wet_right: Knob::new(&params, "wet_right", 0.0),
            tape: Knob::new(&params, "tape", 0.0),
            ping_pong: Knob::new(&params, "ping_pong", 0.0),
            tempo: Knob::new(&params, "tempo", 120.0),
            sync_left: Knob::new(&params, "sync_left", 0.0),
            sync_right: Knob::new(&params, "sync_right", 0.0),
            buffers: [vec![0.0; BUFFER_LEN], vec![0.0; BUFFER_LEN]],
            write: 0,
            current: [0.0; CHANNELS],
            wow_phase: 0.0,
            flutter_phase: 0.0,
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
        self.wow_phase = 0.0;
        self.flutter_phase = 0.0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(CHANNELS);
        let frames = buffer.frames();
        let target = self.target_samples();
        let feedback = (self.feedback.value() / 100.0).clamp(0.0, 0.95);
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let dry = [
            (1.0 - mix) * math::db_to_gain(self.dry_left.value()),
            (1.0 - mix) * math::db_to_gain(self.dry_right.value()),
        ];
        let wet = [
            mix * math::db_to_gain(self.wet_left.value()),
            mix * math::db_to_gain(self.wet_right.value()),
        ];
        let tape = (self.tape.value() / 100.0).clamp(0.0, 1.0);
        let ping_pong = self.ping_pong.flag();
        let rate = self.rate.max(1);
        let glide = math::one_pole_alpha(120.0, rate);
        let wow_step = std::f32::consts::TAU * 0.5 / rate as f32;
        let flutter_step = std::f32::consts::TAU * 6.0 / rate as f32;

        for frame in 0..frames {
            for (current, wanted) in self.current.iter_mut().zip(target.iter()) {
                *current += (*wanted - *current) * glide;
                *current = math::sanitize(*current);
            }
            let mut modulation = 0.0;
            if tape > 0.0 {
                self.wow_phase = (self.wow_phase + wow_step).rem_euclid(std::f32::consts::TAU);
                self.flutter_phase =
                    (self.flutter_phase + flutter_step).rem_euclid(std::f32::consts::TAU);
                let seconds = self.wow_phase.sin() * 0.0015 + self.flutter_phase.sin() * 0.00015;
                modulation = seconds * rate as f32 * tape;
            }
            let write = self.write;
            let mut delayed = [0.0f32; CHANNELS];
            for (channel, slot) in delayed.iter_mut().enumerate() {
                let offset = modulation.min(self.current[channel] * 0.49);
                let position =
                    (write as f32 - self.current[channel] + offset).rem_euclid(BUFFER_LEN as f32);
                let mut value = self.read(channel, position);
                if tape > 0.0 {
                    value = value * (1.0 - tape) + (value * (1.0 + 3.0 * tape)).tanh() * tape;
                }
                *slot = value;
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
                    math::sanitize(input * dry[channel] + delayed[channel] * wet[channel]);
            }
            self.write = (write + 1) % BUFFER_LEN;
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
