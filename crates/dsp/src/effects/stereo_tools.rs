use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const BASE_BUFFER: usize = 8_192;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .step(0.1)
            .group("Input")
            .build(),
        ParamSpec::float("stereo_width", "Stereo width", ParamUnit::Percent)
            .range(0.0, 200.0, 100.0)
            .step(1.0)
            .group("Image")
            .build(),
        ParamSpec::float("balance", "Balance", ParamUnit::Percent)
            .range(-100.0, 100.0, 0.0)
            .step(1.0)
            .group("Image")
            .build(),
        ParamSpec::float("stereo_base", "Stereo base", ParamUnit::Milliseconds)
            .range(0.0, 30.0, 0.0)
            .step(0.1)
            .group("Image")
            .build(),
    ]
}

pub struct StereoTools {
    input_gain: Knob,
    width: Knob,
    balance: Knob,
    base: Knob,
    delay_buffer: Vec<f32>,
    delay_index: usize,
    rate: u32,
}

impl StereoTools {
    pub fn new(params: ParamSet) -> Self {
        Self {
            input_gain: Knob::new(&params, "input_gain", 0.0),
            width: Knob::new(&params, "stereo_width", 100.0),
            balance: Knob::new(&params, "balance", 0.0),
            base: Knob::new(&params, "stereo_base", 0.0),
            delay_buffer: vec![0.0; BASE_BUFFER],
            delay_index: 0,
            rate: 48_000,
        }
    }
}

impl Effect for StereoTools {
    fn id(&self) -> &'static str {
        "stereo_tools"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.reset();
    }

    fn reset(&mut self) {
        self.delay_buffer.fill(0.0);
        self.delay_index = 0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        if channels < 2 {
            let gain = math::db_to_gain(self.input_gain.value());
            for channel in buffer.channels_mut() {
                for sample in channel.iter_mut() {
                    *sample = math::sanitize(*sample * gain);
                }
            }
            return;
        }

        let input_gain = math::db_to_gain(self.input_gain.value());
        let width = (self.width.value() / 100.0).clamp(0.0, 2.0);
        let balance = (self.balance.value() / 100.0).clamp(-1.0, 1.0);
        let left_gain = if balance > 0.0 { 1.0 - balance } else { 1.0 };
        let right_gain = if balance < 0.0 { 1.0 + balance } else { 1.0 };
        let delay = ((self.base.value() / 1000.0) * self.rate.max(1) as f32)
            .clamp(0.0, (BASE_BUFFER - 2) as f32) as usize;
        let frames = buffer.frames();

        for frame in 0..frames {
            let left = buffer.channel_mut(0)[frame] * input_gain;
            let mut right = buffer.channel_mut(1)[frame] * input_gain;
            if delay > 0 {
                let read = (self.delay_index + BASE_BUFFER - delay) % BASE_BUFFER;
                let delayed = self.delay_buffer[read];
                self.delay_buffer[self.delay_index] = right;
                self.delay_index = (self.delay_index + 1) % BASE_BUFFER;
                right = delayed;
            }
            let mid = 0.5 * (left + right);
            let side = 0.5 * (left - right) * width;
            buffer.channel_mut(0)[frame] = math::sanitize((mid + side) * left_gain);
            buffer.channel_mut(1)[frame] = math::sanitize((mid - side) * right_gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo(left: f32, right: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        buffer.channel_mut(0).fill(left);
        buffer.channel_mut(1).fill(right);
        buffer
    }

    #[test]
    fn zero_width_collapses_to_mono() {
        let params = ParamSet::new(specs());
        params.set_f32("stereo_width", 0.0).unwrap();
        let mut effect = StereoTools::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = stereo(1.0, -1.0, 8);
        effect.process(&mut buffer);
        assert!(buffer.channel(0)[0].abs() < 1.0e-6);
        assert!(buffer.channel(1)[0].abs() < 1.0e-6);
    }

    #[test]
    fn double_width_amplifies_the_side_channel() {
        let params = ParamSet::new(specs());
        params.set_f32("stereo_width", 200.0).unwrap();
        let mut effect = StereoTools::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = stereo(1.0, -1.0, 8);
        effect.process(&mut buffer);
        assert!(
            (buffer.channel(0)[0] - 2.0).abs() < 1.0e-5,
            "{}",
            buffer.channel(0)[0]
        );
        assert!((buffer.channel(1)[0] + 2.0).abs() < 1.0e-5);
    }

    #[test]
    fn balance_attenuates_the_opposite_channel() {
        let params = ParamSet::new(specs());
        params.set_f32("balance", 100.0).unwrap();
        let mut effect = StereoTools::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = stereo(0.5, 0.5, 8);
        effect.process(&mut buffer);
        assert!(buffer.channel(0)[0].abs() < 1.0e-6);
    }

    #[test]
    fn stereo_base_shifts_the_right_channel_in_time() {
        let params = ParamSet::new(specs());
        params.set_f32("stereo_base", 1.0).unwrap();
        let mut effect = StereoTools::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = AudioBuffer::new(2, 256);
        buffer.configure(2, 256).unwrap();
        buffer.channel_mut(0)[0] = 1.0;
        buffer.channel_mut(1)[0] = 1.0;
        effect.process(&mut buffer);
        let right_peak = buffer
            .channel(1)
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(index, _)| index)
            .unwrap();
        assert!(right_peak.abs_diff(48) < 4, "right peak at {right_peak}");
    }
}
