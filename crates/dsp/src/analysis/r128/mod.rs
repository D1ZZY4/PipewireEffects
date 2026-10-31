#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use pwe_core::AudioBuffer;

use crate::effects::util::MAX_CHANNELS;

const HOP_MILLISECONDS: u32 = 100;
const BLOCK_HOPS: usize = 4;
const HOP_CAPACITY: usize = 300;
const MOMENTARY_HOPS: usize = 4;
const SHORT_TERM_HOPS: usize = 30;
const ABSOLUTE_GATE_LUFS: f32 = -70.0;
const RELATIVE_GATE_LU: f32 = 10.0;
const OFFSET: f64 = -0.691;

#[derive(Debug, Clone, Copy, Default)]
struct Filter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct State {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl State {
    fn run(&mut self, filter: &Filter, x: f32) -> f32 {
        let y = filter.b0 * x + filter.b1 * self.x1 + filter.b2 * self.x2
            - filter.a1 * self.y1
            - filter.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

fn shelf(rate: f64) -> Filter {
    let f0 = 1_681.974_450_955_533;
    let gain = 3.999_843_853_973_347;
    let q = 0.707_175_236_955_419_6;
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let high = 10f64.powf(gain / 20.0);
    let band = high.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    Filter {
        b0: ((high + band * k / q + k * k) / a0) as f32,
        b1: (2.0 * (k * k - high) / a0) as f32,
        b2: ((high - band * k / q + k * k) / a0) as f32,
        a1: (2.0 * (k * k - 1.0) / a0) as f32,
        a2: ((1.0 - k / q + k * k) / a0) as f32,
    }
}

fn high_pass(rate: f64) -> Filter {
    let f0 = 38.135_470_876_024_44;
    let q = 0.500_327_037_323_877_3;
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let a0 = 1.0 + k / q + k * k;
    Filter {
        b0: 1.0,
        b1: -2.0,
        b2: 1.0,
        a1: (2.0 * (k * k - 1.0) / a0) as f32,
        a2: ((1.0 - k / q + k * k) / a0) as f32,
    }
}

fn loudness(energy: f64) -> f32 {
    if energy <= 0.0 {
        f32::NEG_INFINITY
    } else {
        (OFFSET + 10.0 * energy.log10()) as f32
    }
}

pub struct Loudness {
    shelf: Filter,
    high_pass: Filter,
    shelf_state: [State; MAX_CHANNELS],
    high_pass_state: [State; MAX_CHANNELS],
    hop_frames: usize,
    counted: usize,
    energy: f64,
    hops: [f64; HOP_CAPACITY],
    blocks: [f64; HOP_CAPACITY],
    head: usize,
    len: usize,
    integrated: Option<f32>,
}

impl Loudness {
    pub fn new(rate: u32) -> Self {
        let rate = f64::from(rate.max(1));
        Self {
            shelf: shelf(rate),
            high_pass: high_pass(rate),
            shelf_state: [State::default(); MAX_CHANNELS],
            high_pass_state: [State::default(); MAX_CHANNELS],
            hop_frames: ((rate as u32 / 1000).max(1) * HOP_MILLISECONDS) as usize,
            counted: 0,
            energy: 0.0,
            hops: [0.0; HOP_CAPACITY],
            blocks: [0.0; HOP_CAPACITY],
            head: 0,
            len: 0,
            integrated: None,
        }
    }

    pub fn reset(&mut self) {
        for state in &mut self.shelf_state {
            state.reset();
        }
        for state in &mut self.high_pass_state {
            state.reset();
        }
        self.counted = 0;
        self.energy = 0.0;
        self.head = 0;
        self.len = 0;
        self.integrated = None;
    }

    pub fn process(&mut self, buffer: &AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        for index in 0..buffer.frames() {
            for channel in 0..channels {
                let sample = buffer.channel(channel)[index];
                let weighted = self.high_pass_state[channel].run(
                    &self.high_pass,
                    self.shelf_state[channel].run(&self.shelf, sample),
                );
                let value = f64::from(weighted);
                self.energy += value * value;
            }
            self.counted += 1;
            if self.counted >= self.hop_frames {
                self.finish_hop();
            }
        }
    }

    pub fn integrated(&self) -> Option<f32> {
        self.integrated
    }

    pub fn momentary(&self) -> Option<f32> {
        self.window_loudness(MOMENTARY_HOPS)
    }

    pub fn short_term(&self) -> Option<f32> {
        self.window_loudness(SHORT_TERM_HOPS)
    }

    fn window_loudness(&self, hops: usize) -> Option<f32> {
        if self.len < hops || hops == 0 {
            return None;
        }
        let mut sum = 0.0;
        for offset in 0..hops {
            sum += self.hop(self.len - hops + offset);
        }
        Some(loudness(sum / hops as f64))
    }

    fn finish_hop(&mut self) {
        let frames = self.counted.max(1) as f64;
        self.hops[self.head] = self.energy / frames;
        self.head = (self.head + 1) % HOP_CAPACITY;
        self.len = (self.len + 1).min(HOP_CAPACITY);
        self.counted = 0;
        self.energy = 0.0;
        self.recompute();
    }

    fn hop(&self, index: usize) -> f64 {
        let start = (self.head + HOP_CAPACITY - self.len) % HOP_CAPACITY;
        self.hops[(start + index) % HOP_CAPACITY]
    }

    fn recompute(&mut self) {
        if self.len < BLOCK_HOPS {
            self.integrated = None;
            return;
        }
        let blocks = self.len - BLOCK_HOPS + 1;
        for start in 0..blocks {
            let mut sum = 0.0;
            for offset in 0..BLOCK_HOPS {
                sum += self.hop(start + offset);
            }
            self.blocks[start] = sum / BLOCK_HOPS as f64;
        }

        let (gated_sum, gated_count) = self.gate(blocks, f32::NEG_INFINITY, true);
        if gated_count == 0 {
            self.integrated = None;
            return;
        }
        let threshold = loudness(gated_sum / gated_count as f64) - RELATIVE_GATE_LU;
        let (sum, count) = self.gate(blocks, threshold, false);
        self.integrated = (count > 0).then(|| loudness(sum / count as f64));
    }

    fn gate(&self, blocks: usize, threshold: f32, absolute: bool) -> (f64, usize) {
        let mut sum = 0.0;
        let mut count = 0usize;
        for index in 0..blocks {
            let energy = self.blocks[index];
            let keep = if absolute {
                loudness(energy) > ABSOLUTE_GATE_LUFS
            } else {
                loudness(energy) > threshold
            };
            if keep {
                sum += energy;
                count += 1;
            }
        }
        (sum, count)
    }
}
