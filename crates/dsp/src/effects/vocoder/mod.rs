#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::f32::consts::TAU;

use pwe_core::{AudioBuffer, math};

use crate::fft::{Fft, hann};

pub const CHANNELS: usize = 2;
pub const FFT_SIZE: usize = 2048;
pub const HOP: usize = FFT_SIZE / 4;
const BINS: usize = FFT_SIZE / 2 + 1;
const WINDOW_SUM: f32 = 1.5;

pub const LATENCY_FRAMES: usize = FFT_SIZE;

pub struct Vocoder {
    fft: Fft,
    window: Vec<f32>,
    input: [[f32; FFT_SIZE]; CHANNELS],
    accumulate: [[f32; FFT_SIZE]; CHANNELS],
    pending: [[f32; HOP]; CHANNELS],
    re: [[f32; FFT_SIZE]; CHANNELS],
    im: [[f32; FFT_SIZE]; CHANNELS],
    analysis: [[f32; BINS]; CHANNELS],
    synthesis: [[f32; BINS]; CHANNELS],
    magnitude: [f32; BINS],
    frequency: [f32; BINS],
    filled: usize,
    pending_len: usize,
    pending_read: usize,
}

impl Default for Vocoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Vocoder {
    pub fn new() -> Self {
        Self {
            fft: Fft::new(FFT_SIZE),
            window: hann(FFT_SIZE),
            input: [[0.0; FFT_SIZE]; CHANNELS],
            accumulate: [[0.0; FFT_SIZE]; CHANNELS],
            pending: [[0.0; HOP]; CHANNELS],
            re: [[0.0; FFT_SIZE]; CHANNELS],
            im: [[0.0; FFT_SIZE]; CHANNELS],
            analysis: [[0.0; BINS]; CHANNELS],
            synthesis: [[0.0; BINS]; CHANNELS],
            magnitude: [0.0; BINS],
            frequency: [0.0; BINS],
            filled: 0,
            pending_len: 0,
            pending_read: 0,
        }
    }

    pub fn reset(&mut self) {
        for channel in 0..CHANNELS {
            self.input[channel].fill(0.0);
            self.accumulate[channel].fill(0.0);
            self.pending[channel].fill(0.0);
            self.re[channel].fill(0.0);
            self.im[channel].fill(0.0);
            self.analysis[channel].fill(0.0);
            self.synthesis[channel].fill(0.0);
        }
        self.magnitude.fill(0.0);
        self.frequency.fill(0.0);
        self.filled = 0;
        self.pending_len = 0;
        self.pending_read = 0;
    }

    pub fn process(&mut self, buffer: &mut AudioBuffer, ratio: f32, mix: f32) {
        let channels = buffer.channels().min(CHANNELS);
        let frames = buffer.frames();
        let mix = mix.clamp(0.0, 1.0);
        let mut position = 0;
        while position < frames {
            let take = (HOP - self.filled).min(frames - position);
            let base = FFT_SIZE - HOP + self.filled;
            for channel in 0..channels {
                let plan = buffer.channel(channel);
                self.input[channel][base..base + take]
                    .copy_from_slice(&plan[position..position + take]);
            }
            let read = self.pending_read;
            let length = self.pending_len;
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                for offset in 0..take {
                    let dry = plan[position + offset];
                    let index = read + offset;
                    let wet = if index < length {
                        self.pending[channel][index]
                    } else {
                        0.0
                    };
                    plan[position + offset] = math::sanitize(dry * (1.0 - mix) + wet * mix);
                }
            }
            self.pending_read += take;
            self.filled += take;
            position += take;
            if self.filled == HOP {
                self.filled = 0;
                self.pending_read = 0;
                self.frame(channels, ratio);
                for channel in 0..channels {
                    self.pending[channel].copy_from_slice(&self.accumulate[channel][..HOP]);
                }
                self.pending_len = HOP;
            } else if self.pending_read >= self.pending_len {
                self.pending_read = 0;
                self.pending_len = 0;
            }
        }
    }

    fn frame(&mut self, channels: usize, ratio: f32) {
        for channel in 0..channels {
            for index in 0..FFT_SIZE {
                self.re[channel][index] = self.input[channel][index] * self.window[index];
                self.im[channel][index] = 0.0;
            }
            self.fft
                .forward(&mut self.re[channel], &mut self.im[channel]);
            self.analyse(channel, ratio);
            for bin in 0..BINS {
                if self.magnitude[bin] <= 0.0 {
                    self.re[channel][bin] = 0.0;
                    self.im[channel][bin] = 0.0;
                    continue;
                }
                let advance = TAU * HOP as f32 * self.frequency[bin] / FFT_SIZE as f32;
                self.synthesis[channel][bin] = wrap(self.synthesis[channel][bin] + advance);
                let phase = self.synthesis[channel][bin];
                self.re[channel][bin] = self.magnitude[bin] * phase.cos();
                self.im[channel][bin] = self.magnitude[bin] * phase.sin();
            }
            self.im[channel][0] = 0.0;
            self.im[channel][BINS - 1] = 0.0;
            for bin in BINS..FFT_SIZE {
                self.re[channel][bin] = 0.0;
                self.im[channel][bin] = 0.0;
            }
            for bin in 1..BINS - 1 {
                self.re[channel][FFT_SIZE - bin] = self.re[channel][bin];
                self.im[channel][FFT_SIZE - bin] = -self.im[channel][bin];
            }
            self.fft
                .inverse(&mut self.re[channel], &mut self.im[channel]);
            for index in 0..FFT_SIZE {
                self.accumulate[channel][index] +=
                    self.re[channel][index] * self.window[index] / WINDOW_SUM;
            }
            self.accumulate[channel].copy_within(HOP.., 0);
            self.accumulate[channel][FFT_SIZE - HOP..].fill(0.0);
        }
    }

    fn analyse(&mut self, channel: usize, ratio: f32) {
        self.magnitude.fill(0.0);
        self.frequency.fill(0.0);
        let scale = FFT_SIZE as f32 / (TAU * HOP as f32);
        for bin in 0..BINS {
            let real = self.re[channel][bin];
            let imaginary = self.im[channel][bin];
            let magnitude = (real * real + imaginary * imaginary).sqrt();
            let phase = imaginary.atan2(real);
            let delta = wrap(phase - self.analysis[channel][bin]);
            self.analysis[channel][bin] = phase;
            let expected = TAU * bin as f32 * HOP as f32 / FFT_SIZE as f32;
            let deviation = wrap(delta - expected);
            let shifted = (bin as f32 + deviation * scale) * ratio;
            if shifted < 0.0 || shifted >= BINS as f32 {
                continue;
            }
            let target = shifted.round() as usize;
            if target >= BINS {
                continue;
            }
            if magnitude > self.magnitude[target] {
                self.frequency[target] = shifted;
            }
            self.magnitude[target] += magnitude;
        }
    }
}

pub fn wrap(phase: f32) -> f32 {
    phase - TAU * (phase / TAU).round()
}
