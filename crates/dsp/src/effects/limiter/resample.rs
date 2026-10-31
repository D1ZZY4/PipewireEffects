use crate::effects::util::MAX_CHANNELS;

const TAPS: usize = 16;
const MAX_COEFFS: usize = TAPS * 4;

pub struct Resampler {
    factor: usize,
    length: usize,
    h: [f32; MAX_COEFFS],
    g: [f32; MAX_COEFFS],
    base: [[f32; TAPS]; MAX_CHANNELS],
    decim: [[f32; MAX_COEFFS]; MAX_CHANNELS],
    decim_head: usize,
}

impl Resampler {
    pub fn new() -> Self {
        let mut resampler = Self {
            factor: 1,
            length: TAPS,
            h: [0.0; MAX_COEFFS],
            g: [0.0; MAX_COEFFS],
            base: [[0.0; TAPS]; MAX_CHANNELS],
            decim: [[0.0; MAX_COEFFS]; MAX_CHANNELS],
            decim_head: 0,
        };
        resampler.set_factor(1);
        resampler
    }

    pub fn set_factor(&mut self, factor: usize) {
        self.factor = factor.clamp(1, 4);
        let length = TAPS * self.factor;
        self.length = length;
        let center = (length as f32 - 1.0) / 2.0;
        let mut sum = 0.0f32;
        for index in 0..length {
            let offset = (index as f32 - center) / self.factor as f32;
            let sinc = if offset.abs() < 1.0e-6 {
                1.0
            } else {
                let angle = std::f32::consts::PI * offset;
                angle.sin() / angle
            };
            let phase = index as f32 / (length - 1) as f32;
            let window = 0.42 - 0.5 * (std::f32::consts::TAU * phase).cos()
                + 0.08 * (2.0 * std::f32::consts::TAU * phase).cos();
            self.h[index] = sinc * window;
            sum += self.h[index];
        }
        let scale = self.factor as f32 / sum.max(1.0e-9);
        for index in 0..length {
            self.h[index] *= scale;
            self.g[index] = self.h[index] / self.factor as f32;
        }
        for index in length..MAX_COEFFS {
            self.h[index] = 0.0;
            self.g[index] = 0.0;
        }
        self.reset();
    }

    pub fn reset(&mut self) {
        for history in &mut self.base {
            history.fill(0.0);
        }
        for history in &mut self.decim {
            history.fill(0.0);
        }
        self.decim_head = 0;
    }

    pub fn latency(&self) -> usize {
        TAPS
    }

    pub fn push(&mut self, channel: usize, sample: f32) {
        let history = &mut self.base[channel.min(MAX_CHANNELS - 1)];
        history.copy_within(0..TAPS - 1, 1);
        history[0] = sample;
    }

    pub fn interpolate(&self, channel: usize, phase: usize) -> f32 {
        let history = &self.base[channel.min(MAX_CHANNELS - 1)];
        let mut sum = 0.0f32;
        for (tap, value) in history.iter().enumerate() {
            sum += self.h[phase + tap * self.factor] * value;
        }
        sum
    }

    pub fn push_decim(&mut self, channels: usize, samples: &[f32; MAX_CHANNELS]) {
        self.decim_head = (self.decim_head + 1) % self.length;
        for (channel, history) in self.decim.iter_mut().enumerate().take(channels) {
            history[self.decim_head] = samples[channel];
        }
    }

    pub fn decimate(&self, channel: usize) -> f32 {
        let history = &self.decim[channel.min(MAX_CHANNELS - 1)];
        let mut sum = 0.0f32;
        for tap in 0..self.length {
            let index = (self.decim_head + self.length - tap) % self.length;
            sum += self.g[tap] * history[index];
        }
        sum
    }
}
