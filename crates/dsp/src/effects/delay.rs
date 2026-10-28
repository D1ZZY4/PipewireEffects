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
    ]
}

pub struct Delay {
    delay_left: Knob,
    delay_right: Knob,
    feedback: Knob,
    mix: Knob,
    ping_pong: Knob,
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
            buffers: [vec![0.0; BUFFER_LEN], vec![0.0; BUFFER_LEN]],
            write: 0,
            current: [0.0; CHANNELS],
            rate: 48_000,
        }
    }

    fn target_samples(&self) -> [f32; CHANNELS] {
        let scale = self.rate.max(1) as f32 / 1000.0;
        let limit = BUFFER_LEN as f32 - 4.0;
        [
            (self.delay_left.value() * scale).clamp(1.0, limit),
            (self.delay_right.value() * scale).clamp(1.0, limit),
        ]
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
mod tests {
    use super::*;

    fn impulse(frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        buffer.channel_mut(0)[0] = 1.0;
        buffer.channel_mut(1)[0] = 1.0;
        buffer
    }

    #[test]
    fn a_delayed_impulse_reappears_after_the_delay_time() {
        let params = ParamSet::new(specs());
        params.set_f32("delay_left", 10.0).unwrap();
        params.set_f32("delay_right", 10.0).unwrap();
        params.set_f32("mix", 100.0).unwrap();
        params.set_f32("feedback", 0.0).unwrap();
        let mut effect = Delay::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = impulse(2048);
        effect.process(&mut buffer);
        let plan = buffer.channel(0);
        let peak_index = plan
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(index, _)| index)
            .unwrap();
        assert!(plan[peak_index].abs() > 0.5, "no echo found");
        assert!(peak_index.abs_diff(480) < 90, "echo at {peak_index}");
    }

    #[test]
    fn feedback_decays_and_never_blows_up() {
        let params = ParamSet::new(specs());
        params.set_f32("feedback", 95.0).unwrap();
        params.set_f32("mix", 100.0).unwrap();
        let mut effect = Delay::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = impulse(1024);
        effect.process(&mut buffer);
        for _ in 0..2000 {
            let mut buffer = impulse(1024);
            buffer.channel_mut(0)[0] = 0.0;
            buffer.channel_mut(1)[0] = 0.0;
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 2.0, "{sample}");
                }
            }
        }
    }

    #[test]
    fn dry_signal_passes_through_with_zero_mix() {
        let params = ParamSet::new(specs());
        params.set_f32("mix", 0.0).unwrap();
        let mut effect = Delay::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = impulse(512);
        effect.process(&mut buffer);
        assert!((buffer.channel(0)[0] - 1.0).abs() < 1.0e-6);
    }
}
