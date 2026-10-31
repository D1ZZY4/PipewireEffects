use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const BASE_BUFFER: usize = 8_192;
const MODES: [&str; 7] = [
    mark("Stereo"),
    mark("Stereo to mid/side"),
    mark("Mid/side to stereo"),
    mark("Mono, left"),
    mark("Mono, right"),
    mark("Mono sum"),
    mark("Swap channels"),
];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("mode", "Mode", &MODES, 0).group("Input"),
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .step(0.1)
            .group("Input")
            .build(),
        ParamSpec::float("balance_in", "Balance in", ParamUnit::Percent)
            .range(-100.0, 100.0, 0.0)
            .step(1.0)
            .group("Input")
            .build(),
        ParamSpec::boolean("phase_left", "Invert left phase", false).group("Input"),
        ParamSpec::boolean("phase_right", "Invert right phase", false).group("Input"),
        ParamSpec::boolean("mute_left", "Mute left", false).group("Input"),
        ParamSpec::boolean("mute_right", "Mute right", false).group("Input"),
        ParamSpec::float("middle_level", "Middle level", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Middle and side")
            .build(),
        ParamSpec::float("side_level", "Side level", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Middle and side")
            .build(),
        ParamSpec::float("middle_panorama", "Middle panorama", ParamUnit::Percent)
            .range(-100.0, 100.0, 0.0)
            .step(1.0)
            .group("Middle and side")
            .build(),
        ParamSpec::float("side_balance", "Side balance", ParamUnit::Percent)
            .range(-100.0, 100.0, 0.0)
            .step(1.0)
            .group("Middle and side")
            .build(),
        ParamSpec::float("stereo_width", "Stereo width", ParamUnit::Percent)
            .range(0.0, 200.0, 100.0)
            .step(1.0)
            .group("Image")
            .build(),
        ParamSpec::float("delay", "Delay", ParamUnit::Milliseconds)
            .range(0.0, 30.0, 0.0)
            .step(0.1)
            .group("Image")
            .build(),
        ParamSpec::float("stereo_phase", "Stereo phase", ParamUnit::None)
            .range(0.0, 360.0, 0.0)
            .step(1.0)
            .group("Image")
            .build(),
        ParamSpec::float("balance_out", "Balance out", ParamUnit::Percent)
            .range(-100.0, 100.0, 0.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::float("sc_level", "Side-chain level", ParamUnit::Percent)
            .range(1.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::boolean("softclip", "Soft clip", false).group("Output"),
    ]
}

fn balance(gain: &mut [f32; 2], amount: f32) {
    if amount > 0.0 {
        gain[0] *= 1.0 - amount;
    } else if amount < 0.0 {
        gain[1] *= 1.0 + amount;
    }
}

pub struct StereoTools {
    params: [Knob; 17],
    delay_buffer: Vec<f32>,
    delay_index: usize,
    rate: u32,
}

impl StereoTools {
    pub fn new(params: ParamSet) -> Self {
        Self {
            params: [
                Knob::new(&params, "mode", 0.0),
                Knob::new(&params, "input_gain", 0.0),
                Knob::new(&params, "balance_in", 0.0),
                Knob::new(&params, "phase_left", 0.0),
                Knob::new(&params, "phase_right", 0.0),
                Knob::new(&params, "mute_left", 0.0),
                Knob::new(&params, "mute_right", 0.0),
                Knob::new(&params, "middle_level", 0.0),
                Knob::new(&params, "side_level", 0.0),
                Knob::new(&params, "middle_panorama", 0.0),
                Knob::new(&params, "side_balance", 0.0),
                Knob::new(&params, "stereo_width", 100.0),
                Knob::new(&params, "delay", 0.0),
                Knob::new(&params, "balance_out", 0.0),
                Knob::new(&params, "softclip", 0.0),
                Knob::new(&params, "stereo_phase", 0.0),
                Knob::new(&params, "sc_level", 100.0),
            ],
            delay_buffer: vec![0.0; BASE_BUFFER],
            delay_index: 0,
            rate: 48_000,
        }
    }

    fn route(&self, mode: usize, pair: [f32; 2]) -> [f32; 2] {
        let [left, right] = pair;
        let middle = 0.5 * (left + right);
        let side = 0.5 * (left - right);
        match mode {
            1 => [middle, side],
            2 => [left + right, left - right],
            3 => [left, left],
            4 => [right, right],
            5 => [middle, middle],
            6 => [right, left],
            _ => [left, right],
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
        let input_gain = math::db_to_gain(self.params[1].value());
        if channels < 2 {
            for channel in buffer.channels_mut() {
                for sample in channel.iter_mut() {
                    *sample = math::sanitize(*sample * input_gain);
                }
            }
            return;
        }

        let mode = self.params[0].index().min(MODES.len() - 1);
        let balance_in = (self.params[2].value() / 100.0).clamp(-1.0, 1.0);
        let phase = [self.params[3].flag(), self.params[4].flag()];
        let mute = [self.params[5].flag(), self.params[6].flag()];
        let middle_gain = math::db_to_gain(self.params[7].value());
        let side_gain = math::db_to_gain(self.params[8].value());
        let panorama = (self.params[9].value() / 100.0).clamp(-1.0, 1.0);
        let side_balance = (self.params[10].value() / 100.0).clamp(-1.0, 1.0);
        let width = (self.params[11].value() / 100.0).clamp(0.0, 2.0);
        let balance_out = (self.params[13].value() / 100.0).clamp(-1.0, 1.0);
        let softclip = self.params[14].flag();
        let stereo_phase = self.params[15].value().clamp(0.0, 360.0).to_radians();
        let sc_drive = (self.params[16].value() / 100.0).clamp(0.01, 1.0);
        let delay = ((self.params[12].value() / 1000.0) * self.rate.max(1) as f32)
            .clamp(0.0, (BASE_BUFFER - 2) as f32) as usize;
        let frames = buffer.frames();

        for frame in 0..frames {
            let mut pair = [
                buffer.channel_mut(0)[frame] * input_gain,
                buffer.channel_mut(1)[frame] * input_gain,
            ];
            let mut input_gains = [1.0f32; 2];
            balance(&mut input_gains, balance_in);
            for (value, gain) in pair.iter_mut().zip(input_gains) {
                *value *= gain;
            }
            for (value, inverted) in pair.iter_mut().zip(phase) {
                if inverted {
                    *value = -*value;
                }
            }
            for (value, muted) in pair.iter_mut().zip(mute) {
                if muted {
                    *value = 0.0;
                }
            }
            let mut routed = self.route(mode, pair);
            if delay > 0 {
                let read = (self.delay_index + BASE_BUFFER - delay) % BASE_BUFFER;
                let delayed = self.delay_buffer[read];
                self.delay_buffer[self.delay_index] = routed[1];
                self.delay_index = (self.delay_index + 1) % BASE_BUFFER;
                routed[1] = delayed;
            }
            let middle = 0.5 * (routed[0] + routed[1]) * middle_gain;
            let side = 0.5 * (routed[0] - routed[1]) * side_gain * width;
            let mut middle_pair = [middle, middle];
            let mut side_pair = [side, -side];
            balance(&mut middle_pair, panorama);
            balance(&mut side_pair, side_balance);
            let mut output = [middle_pair[0] + side_pair[0], middle_pair[1] + side_pair[1]];
            if stereo_phase != 0.0 {
                let (sin, cos) = stereo_phase.sin_cos();
                output = [
                    output[0] * cos - output[1] * sin,
                    output[0] * sin + output[1] * cos,
                ];
            }
            let mut output_gains = [1.0f32; 2];
            balance(&mut output_gains, balance_out);
            for (value, gain) in output.iter_mut().zip(output_gains) {
                *value *= gain;
            }
            for (channel, value) in output.iter().enumerate() {
                let value = if softclip {
                    (*value * sc_drive).tanh()
                } else {
                    *value
                };
                buffer.channel_mut(channel)[frame] = math::sanitize(value);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
