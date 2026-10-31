use pwe_core::AudioBuffer;
use pwe_core::math;

use crate::effects::util::MAX_CHANNELS;
use crate::fft::{Fft, hann};

const FFT_SIZE: usize = 512;
const HOP: usize = FFT_SIZE / 4;
const BINS: usize = FFT_SIZE / 2 + 1;
const WINDOW_SUM: f32 = 1.5;
const PEAK_DECAY: f32 = 0.98;
const GAIN_FLOOR: f32 = 0.1;

pub const LATENCY_FRAMES: usize = FFT_SIZE - HOP;

pub struct Dereverb {
    fft: Fft,
    window: Vec<f32>,
    input: [[f32; FFT_SIZE]; MAX_CHANNELS],
    output: [[f32; FFT_SIZE]; MAX_CHANNELS],
    re: [[f32; FFT_SIZE]; MAX_CHANNELS],
    im: [[f32; FFT_SIZE]; MAX_CHANNELS],
    peak: [[f32; BINS]; MAX_CHANNELS],
}

impl Dereverb {
    pub fn new() -> Self {
        Self {
            fft: Fft::new(FFT_SIZE),
            window: hann(FFT_SIZE),
            input: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            output: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            re: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            im: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            peak: [[0.0; BINS]; MAX_CHANNELS],
        }
    }

    pub fn reset(&mut self) {
        self.input = [[0.0; FFT_SIZE]; MAX_CHANNELS];
        self.output = [[0.0; FFT_SIZE]; MAX_CHANNELS];
        self.peak = [[0.0; BINS]; MAX_CHANNELS];
    }

    pub fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();
        let mut position = 0;
        while position < frames {
            let take = HOP.min(frames - position);
            for channel in 0..channels {
                let plan = buffer.channel(channel);
                self.input[channel][FFT_SIZE - HOP..FFT_SIZE - HOP + take]
                    .copy_from_slice(&plan[position..position + take]);
            }
            self.transform(channels);
            self.synthesize(channels);
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                for index in 0..take {
                    plan[position + index] = math::sanitize(self.output[channel][index]);
                }
            }
            self.advance(channels);
            position += take;
        }
    }

    fn transform(&mut self, channels: usize) {
        for channel in 0..channels {
            for (index, slot) in self.re[channel].iter_mut().enumerate() {
                *slot = self.input[channel][index] * self.window[index];
            }
            self.im[channel].fill(0.0);
        }
        for channel in 0..channels {
            self.fft
                .forward(&mut self.re[channel], &mut self.im[channel]);
            for bin in 0..BINS {
                let magnitude = self.re[channel][bin].hypot(self.im[channel][bin]);
                let peak = self.peak[channel][bin];
                let gain = if peak > 1.0e-9 {
                    (magnitude / peak).clamp(GAIN_FLOOR, 1.0)
                } else {
                    1.0
                };
                self.re[channel][bin] *= gain;
                self.im[channel][bin] *= gain;
                self.peak[channel][bin] = magnitude.max(peak * PEAK_DECAY);
            }
            self.im[channel][0] = 0.0;
            self.im[channel][FFT_SIZE / 2] = 0.0;
        }
    }

    fn synthesize(&mut self, channels: usize) {
        for channel in 0..channels {
            self.fft
                .inverse(&mut self.re[channel], &mut self.im[channel]);
            for (index, slot) in self.output[channel].iter_mut().enumerate() {
                *slot += self.re[channel][index] * self.window[index] / WINDOW_SUM;
            }
        }
    }

    fn advance(&mut self, channels: usize) {
        for channel in 0..channels {
            self.input[channel].copy_within(HOP.., 0);
            self.output[channel].copy_within(HOP.., 0);
            self.input[channel][FFT_SIZE - HOP..].fill(0.0);
            self.output[channel][FFT_SIZE - HOP..].fill(0.0);
        }
    }
}
