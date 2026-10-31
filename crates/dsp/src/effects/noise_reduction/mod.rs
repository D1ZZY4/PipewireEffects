use nnnoiseless::DenoiseState;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub const SUPPORTED_RATE: u32 = 48_000;
const FRAME: usize = 480;
const SCALE: f32 = 32_768.0;
const RELEASE_COEFFICIENT: f32 = 0.3935;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::boolean("vad", "Voice activity gate", false).group("Detection"),
        ParamSpec::float("vad_threshold", "Gate threshold", ParamUnit::Percent)
            .range(0.0, 100.0, 50.0)
            .step(1.0)
            .group("Detection")
            .build(),
    ]
}

struct Channel {
    state: Box<DenoiseState<'static>>,
    frame_in: [f32; FRAME],
    frame_out: [f32; FRAME],
    queue: [f32; FRAME],
    filled: usize,
    ready: usize,
    head: usize,
    gate: f32,
    first: bool,
}

impl Channel {
    fn new() -> Self {
        Self {
            state: DenoiseState::new(),
            frame_in: [0.0; FRAME],
            frame_out: [0.0; FRAME],
            queue: [0.0; FRAME],
            filled: 0,
            ready: 0,
            head: 0,
            gate: 1.0,
            first: true,
        }
    }

    fn reset(&mut self) {
        self.frame_in = [0.0; FRAME];
        self.frame_out = [0.0; FRAME];
        self.queue = [0.0; FRAME];
        self.filled = 0;
        self.ready = 0;
        self.head = 0;
        self.gate = 1.0;
        self.first = true;
    }

    fn push(&mut self, input: f32, gate_enabled: bool, threshold: f32) -> f32 {
        self.frame_in[self.filled] = input * SCALE;
        self.filled += 1;
        if self.filled >= FRAME {
            let vad = self
                .state
                .process_frame(&mut self.frame_out, &self.frame_in);
            if self.first {
                self.frame_out.copy_from_slice(&self.frame_in);
                self.first = false;
            } else {
                let target = f32::from(!(gate_enabled && vad < threshold));
                self.gate += (target - self.gate) * RELEASE_COEFFICIENT;
            }
            for (slot, value) in self.queue.iter_mut().zip(&self.frame_out) {
                *slot = value / SCALE * self.gate;
            }
            self.filled = 0;
            self.ready = FRAME;
            self.head = 0;
        }
        if self.ready > 0 {
            let value = self.queue[self.head];
            self.head = (self.head + 1) % FRAME;
            self.ready -= 1;
            value
        } else {
            0.0
        }
    }
}

fn warm_up() {
    let mut state = DenoiseState::new();
    let input = [0.0f32; FRAME];
    let mut output = [0.0f32; FRAME];
    let _ = state.process_frame(&mut output, &input);
}

pub struct NoiseReduction {
    channels: Vec<Channel>,
    active: usize,
    supported: bool,
    input_gain: Knob,
    output_gain: Knob,
    mix: Knob,
    vad: Knob,
    threshold: Knob,
}

impl NoiseReduction {
    pub fn new(params: ParamSet) -> Self {
        warm_up();
        Self {
            channels: (0..MAX_CHANNELS).map(|_| Channel::new()).collect(),
            active: 2,
            supported: true,
            input_gain: Knob::new(&params, "input_gain", 0.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            vad: Knob::new(&params, "vad", 0.0),
            threshold: Knob::new(&params, "vad_threshold", 50.0),
        }
    }
}

impl Effect for NoiseReduction {
    fn id(&self) -> &'static str {
        "noise_reduction"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.active = format.channels().min(self.channels.len()).max(1);
        self.supported = format.rate() == SUPPORTED_RATE;
        for channel in &mut self.channels {
            channel.reset();
        }
    }

    fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let input_gain = math::db_to_gain(self.input_gain.value());
        let output_gain = math::db_to_gain(self.output_gain.value());
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let gate_enabled = self.vad.flag();
        let threshold = (self.threshold.value() / 100.0).clamp(0.0, 1.0);
        let supported = self.supported;
        let channels = buffer.channels().min(self.active).min(self.channels.len());
        for index in 0..channels {
            let channel = &mut self.channels[index];
            for sample in buffer.channel_mut(index) {
                let dry = math::sanitize(*sample * input_gain);
                let wet = if supported {
                    channel.push(dry, gate_enabled, threshold)
                } else {
                    dry
                };
                *sample = math::sanitize((dry * (1.0 - mix) + wet * mix) * output_gain);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
