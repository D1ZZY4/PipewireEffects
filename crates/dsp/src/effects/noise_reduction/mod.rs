use nnnoiseless::DenoiseState;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

mod resample;

use resample::{LinearResampler, MAX_OUTPUT};

pub const SUPPORTED_RATE: u32 = 48_000;
const FRAME: usize = 480;
const SCALE: f32 = 32_768.0;
const FIFO: usize = 64;
const GATE_SMOOTHING: f32 = 0.3935;

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
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(0.0, 20_000.0, 20.0)
            .step(1.0)
            .group("Detection")
            .build(),
    ]
}

fn release_frames(release_ms: f32) -> usize {
    ((SUPPORTED_RATE as f32 * release_ms.max(0.0) / 1000.0) / FRAME as f32).round() as usize
}

fn gate_target(high: bool, hold: &mut usize, release: usize) -> f32 {
    if high {
        *hold = release + 1;
    }
    let open = high || *hold > 0;
    *hold = hold.saturating_sub(1);
    if open { 1.0 } else { 0.0 }
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
    hold: usize,
    first: bool,
    direct: bool,
    in_resampler: LinearResampler,
    out_resampler: LinearResampler,
    fifo: [f32; FIFO],
    fifo_head: usize,
    fifo_len: usize,
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
            hold: 0,
            first: true,
            direct: true,
            in_resampler: LinearResampler::new(SUPPORTED_RATE, SUPPORTED_RATE),
            out_resampler: LinearResampler::new(SUPPORTED_RATE, SUPPORTED_RATE),
            fifo: [0.0; FIFO],
            fifo_head: 0,
            fifo_len: 0,
        }
    }

    fn configure(&mut self, rate: u32) {
        self.direct = rate == SUPPORTED_RATE;
        self.in_resampler = LinearResampler::new(rate, SUPPORTED_RATE);
        self.out_resampler = LinearResampler::new(SUPPORTED_RATE, rate);
        self.reset();
    }

    fn reset(&mut self) {
        self.frame_in = [0.0; FRAME];
        self.frame_out = [0.0; FRAME];
        self.queue = [0.0; FRAME];
        self.filled = 0;
        self.ready = 0;
        self.head = 0;
        self.gate = 1.0;
        self.hold = 0;
        self.first = true;
        self.fifo = [0.0; FIFO];
        self.fifo_head = 0;
        self.fifo_len = 0;
        self.in_resampler.reset();
        self.out_resampler.reset();
    }

    fn ingest(
        &mut self,
        sample: f32,
        gate_enabled: bool,
        threshold: f32,
        release: usize,
    ) -> Option<f32> {
        self.frame_in[self.filled] = sample;
        self.filled += 1;
        if self.filled >= FRAME {
            let vad = self
                .state
                .process_frame(&mut self.frame_out, &self.frame_in);
            if self.first {
                self.frame_out.copy_from_slice(&self.frame_in);
                self.first = false;
            } else {
                let target = if gate_enabled {
                    gate_target(vad >= threshold, &mut self.hold, release)
                } else {
                    self.hold = 0;
                    1.0
                };
                self.gate += (target - self.gate) * GATE_SMOOTHING;
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
            Some(value)
        } else {
            None
        }
    }

    fn push(&mut self, input: f32, gate_enabled: bool, threshold: f32, release: usize) -> f32 {
        self.ingest(input * SCALE, gate_enabled, threshold, release)
            .unwrap_or(0.0)
    }

    fn push_resampled(&mut self, input: f32, gate_enabled: bool, threshold: f32, release: usize) {
        let mut upsampled = [0.0f32; MAX_OUTPUT];
        let count = self.in_resampler.push(input, &mut upsampled);
        for sample in upsampled.iter().take(count) {
            let Some(value) = self.ingest(*sample * SCALE, gate_enabled, threshold, release) else {
                continue;
            };
            let mut downsampled = [0.0f32; MAX_OUTPUT];
            let emitted = self.out_resampler.push(value, &mut downsampled);
            for value in downsampled.iter().take(emitted) {
                self.push_output(*value);
            }
        }
    }

    fn push_output(&mut self, value: f32) {
        if self.fifo_len >= FIFO {
            self.fifo_head = (self.fifo_head + 1) % FIFO;
            self.fifo_len -= 1;
        }
        let tail = (self.fifo_head + self.fifo_len) % FIFO;
        self.fifo[tail] = value;
        self.fifo_len += 1;
    }

    fn take(&mut self) -> f32 {
        if self.fifo_len == 0 {
            return 0.0;
        }
        let value = self.fifo[self.fifo_head];
        self.fifo_head = (self.fifo_head + 1) % FIFO;
        self.fifo_len -= 1;
        value
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
    input_gain: Knob,
    output_gain: Knob,
    mix: Knob,
    vad: Knob,
    threshold: Knob,
    release: Knob,
}

impl NoiseReduction {
    pub fn new(params: ParamSet) -> Self {
        warm_up();
        Self {
            channels: (0..MAX_CHANNELS).map(|_| Channel::new()).collect(),
            active: 2,
            input_gain: Knob::new(&params, "input_gain", 0.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            vad: Knob::new(&params, "vad", 0.0),
            threshold: Knob::new(&params, "vad_threshold", 50.0),
            release: Knob::new(&params, "release", 20.0),
        }
    }
}

impl Effect for NoiseReduction {
    fn id(&self) -> &'static str {
        "noise_reduction"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.active = format.channels().min(self.channels.len()).max(1);
        for channel in &mut self.channels {
            channel.configure(format.rate());
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
        let release = release_frames(self.release.value());
        let channels = buffer.channels().min(self.active).min(self.channels.len());
        for index in 0..channels {
            let channel = &mut self.channels[index];
            for sample in buffer.channel_mut(index) {
                let dry = math::sanitize(*sample * input_gain);
                let wet = if channel.direct {
                    channel.push(dry, gate_enabled, threshold, release)
                } else {
                    channel.push_resampled(dry, gate_enabled, threshold, release);
                    channel.take()
                };
                *sample = math::sanitize((dry * (1.0 - mix) + wet * mix) * output_gain);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/resample.rs"]
mod resample_tests;
