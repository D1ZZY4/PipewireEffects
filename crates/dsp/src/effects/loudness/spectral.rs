use pwe_core::AudioBuffer;
use pwe_core::math;

use crate::biquad::Coefficients;
use crate::effects::util::MAX_CHANNELS;
use crate::fft::Fft;

const FFT_SIZE: usize = 1_024;
const HOP: usize = FFT_SIZE / 4;
const BINS: usize = FFT_SIZE / 2 + 1;

pub struct SpectralContour {
    fft: Fft,
    response: [f32; FFT_SIZE],
    history: [[f32; FFT_SIZE]; MAX_CHANNELS],
    re: [[f32; FFT_SIZE]; MAX_CHANNELS],
    im: [[f32; FFT_SIZE]; MAX_CHANNELS],
}

impl SpectralContour {
    pub fn new() -> Self {
        Self {
            fft: Fft::new(FFT_SIZE),
            response: [1.0; FFT_SIZE],
            history: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            re: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            im: [[0.0; FFT_SIZE]; MAX_CHANNELS],
        }
    }

    pub fn reset(&mut self) {
        self.history = [[0.0; FFT_SIZE]; MAX_CHANNELS];
    }

    pub fn update(&mut self, low: Coefficients, high: Coefficients, rate: f32) {
        for bin in 0..BINS {
            let frequency = bin as f32 * rate / FFT_SIZE as f32;
            self.response[bin] =
                low.magnitude_at(rate, frequency) * high.magnitude_at(rate, frequency);
        }
        for bin in 1..BINS - 1 {
            self.response[FFT_SIZE - bin] = self.response[bin];
        }
    }

    pub fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();
        let mut position = 0;
        while position < frames {
            let take = HOP.min(frames - position);
            for channel in 0..channels {
                self.history[channel].copy_within(HOP.., 0);
                self.history[channel][FFT_SIZE - HOP..].fill(0.0);
                let plan = buffer.channel(channel);
                self.history[channel][FFT_SIZE - HOP..FFT_SIZE - HOP + take]
                    .copy_from_slice(&plan[position..position + take]);
            }
            for channel in 0..channels {
                self.re[channel].copy_from_slice(&self.history[channel]);
                self.im[channel].fill(0.0);
                self.fft
                    .forward(&mut self.re[channel], &mut self.im[channel]);
                for bin in 0..FFT_SIZE {
                    let gain = self.response[bin];
                    self.re[channel][bin] *= gain;
                    self.im[channel][bin] *= gain;
                }
                self.fft
                    .inverse(&mut self.re[channel], &mut self.im[channel]);
            }
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                for index in 0..take {
                    plan[position + index] =
                        math::sanitize(self.re[channel][FFT_SIZE - HOP + index]);
                }
            }
            position += take;
        }
    }
}
