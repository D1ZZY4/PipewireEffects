use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;
const MAX_SCALE: f32 = 4.4;
const MAX_PRE_DELAY: usize = 38_400;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("room_size", "Room size", ParamUnit::Percent)
            .range(0.0, 100.0, 60.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::float("damping", "Damping", ParamUnit::Percent)
            .range(0.0, 100.0, 50.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::float("pre_delay", "Pre-delay", ParamUnit::Milliseconds)
            .range(0.0, 200.0, 20.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::float("width", "Width", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::float("wet", "Wet", ParamUnit::Decibel)
            .range(-60.0, 0.0, -6.0)
            .step(0.5)
            .group("Output")
            .build(),
        ParamSpec::float("dry", "Dry", ParamUnit::Decibel)
            .range(-60.0, 0.0, 0.0)
            .step(0.5)
            .group("Output")
            .build(),
    ]
}

#[derive(Debug)]
struct Comb {
    buffer: Vec<f32>,
    length: usize,
    index: usize,
    filter_state: f32,
    feedback: f32,
    damping: f32,
}

impl Comb {
    fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            buffer: vec![0.0; capacity],
            length: capacity,
            index: 0,
            filter_state: 0.0,
            feedback: 0.5,
            damping: 0.5,
        }
    }

    fn set_length(&mut self, length: usize) {
        let length = length.clamp(1, self.buffer.len());
        if length != self.length {
            self.length = length;
            self.index %= length;
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filter_state =
            math::sanitize(output * (1.0 - self.damping) + self.filter_state * self.damping);
        self.buffer[self.index] = math::sanitize(input + self.filter_state * self.feedback);
        self.index += 1;
        if self.index >= self.length {
            self.index = 0;
        }
        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.filter_state = 0.0;
        self.index = 0;
    }
}

#[derive(Debug)]
struct AllPass {
    buffer: Vec<f32>,
    length: usize,
    index: usize,
}

impl AllPass {
    fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            buffer: vec![0.0; capacity],
            length: capacity,
            index: 0,
        }
    }

    fn set_length(&mut self, length: usize) {
        let length = length.clamp(1, self.buffer.len());
        if length != self.length {
            self.length = length;
            self.index %= length;
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let output = -input + buffered;
        self.buffer[self.index] = math::sanitize(input + buffered * 0.5);
        self.index += 1;
        if self.index >= self.length {
            self.index = 0;
        }
        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }
}

pub struct Reverb {
    room_size: Knob,
    damping: Knob,
    pre_delay: Knob,
    width: Knob,
    wet: Knob,
    dry: Knob,
    combs: [[Comb; 8]; 2],
    allpasses: [[AllPass; 4]; 2],
    pre_delay_buffer: Vec<f32>,
    pre_delay_index: usize,
    rate: u32,
    dirty: bool,
}

impl Reverb {
    pub fn new(params: ParamSet) -> Self {
        let combs = std::array::from_fn(|channel| {
            std::array::from_fn(|band| {
                let spread = if channel == 1 { STEREO_SPREAD } else { 0 };
                Comb::new(((COMB_TUNING[band] + spread) as f32 * MAX_SCALE) as usize)
            })
        });
        let allpasses = std::array::from_fn(|channel| {
            std::array::from_fn(|band| {
                let spread = if channel == 1 { STEREO_SPREAD } else { 0 };
                AllPass::new(((ALLPASS_TUNING[band] + spread) as f32 * MAX_SCALE) as usize)
            })
        });
        Self {
            room_size: Knob::new(&params, "room_size", 60.0),
            damping: Knob::new(&params, "damping", 50.0),
            pre_delay: Knob::new(&params, "pre_delay", 20.0),
            width: Knob::new(&params, "width", 100.0),
            wet: Knob::new(&params, "wet", -6.0),
            dry: Knob::new(&params, "dry", 0.0),
            combs,
            allpasses,
            pre_delay_buffer: vec![0.0; MAX_PRE_DELAY],
            pre_delay_index: 0,
            rate: 48_000,
            dirty: true,
        }
    }

    fn tune(&mut self) {
        let scale = self.rate.max(1) as f32 / 44_100.0;
        for channel in 0..2 {
            let spread = if channel == 1 { STEREO_SPREAD } else { 0 };
            for (band, tuning) in COMB_TUNING.iter().enumerate() {
                self.combs[channel][band].set_length(((*tuning + spread) as f32 * scale) as usize);
            }
            for (band, tuning) in ALLPASS_TUNING.iter().enumerate() {
                self.allpasses[channel][band]
                    .set_length(((*tuning + spread) as f32 * scale) as usize);
            }
        }
    }
}

impl Effect for Reverb {
    fn id(&self) -> &'static str {
        "reverb"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.combs {
            for comb in channel {
                comb.reset();
            }
        }
        for channel in &mut self.allpasses {
            for allpass in channel {
                allpass.reset();
            }
        }
        self.pre_delay_buffer.fill(0.0);
        self.pre_delay_index = 0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        if self.dirty {
            self.dirty = false;
            self.tune();
        }
        let feedback = 0.7 + 0.28 * (self.room_size.value() / 100.0).clamp(0.0, 1.0);
        let damping = (self.damping.value() / 100.0).clamp(0.0, 0.99) * 0.4;
        let width = (self.width.value() / 100.0).clamp(0.0, 1.0);
        let wet = math::db_to_gain(self.wet.value());
        let dry = math::db_to_gain(self.dry.value());
        let pre_delay = ((self.pre_delay.value() / 1000.0) * self.rate.max(1) as f32)
            .clamp(0.0, MAX_PRE_DELAY as f32 - 1.0) as usize;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for channel in 0..2 {
            for comb in self.combs[channel].iter_mut() {
                comb.feedback = feedback;
                comb.damping = damping;
            }
        }

        for frame in 0..frames {
            let mut wet_signal = [0.0f32; 2];
            for (channel, slot) in wet_signal.iter_mut().enumerate().take(channels.min(2)) {
                let input = buffer.channel_mut(channel)[frame];
                let delayed = if pre_delay > 0 {
                    let read = (self.pre_delay_index + MAX_PRE_DELAY - pre_delay) % MAX_PRE_DELAY;
                    let value = self.pre_delay_buffer[read];
                    self.pre_delay_buffer[self.pre_delay_index] = input;
                    value
                } else {
                    input
                };
                let mut sum = 0.0f32;
                for comb in self.combs[channel].iter_mut() {
                    sum += comb.process(delayed * 0.015);
                }
                for allpass in self.allpasses[channel].iter_mut() {
                    sum = allpass.process(sum);
                }
                *slot = sum;
            }
            if pre_delay > 0 {
                self.pre_delay_index = (self.pre_delay_index + 1) % MAX_PRE_DELAY;
            }
            for (channel, local) in wet_signal.iter().enumerate().take(channels.min(2)) {
                let input = buffer.channel_mut(channel)[frame];
                let mixed = *local * width + wet_signal[1 - channel] * (1.0 - width);
                buffer.channel_mut(channel)[frame] = math::sanitize(input * dry + mixed * wet);
            }
        }
    }
}
