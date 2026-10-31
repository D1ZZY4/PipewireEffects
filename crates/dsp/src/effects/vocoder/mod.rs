#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

mod options;

pub use options::{PhaseMode, TransientDetector, TransientMode, VocoderOptions};

use std::f32::consts::TAU;

use pwe_core::{AudioBuffer, math};

use crate::fft::{Fft, hann};

pub const CHANNELS: usize = 2;
pub const FFT_SIZE: usize = 2048;
pub const HOP: usize = FFT_SIZE / 4;
const BINS: usize = FFT_SIZE / 2 + 1;
const WINDOW_SUM: f32 = 1.5;
const RESET_LOW_BIN: f32 = 3.0;
const RESET_HIGH_BIN: f32 = 51.0;

pub const LATENCY_FRAMES: usize = FFT_SIZE;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reset {
    None,
    All,
    Outside,
}

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
    previous: [f32; BINS],
    flux: f32,
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
            previous: [0.0; BINS],
            flux: 0.0,
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
        self.previous.fill(0.0);
        self.flux = 0.0;
        self.filled = 0;
        self.pending_len = 0;
        self.pending_read = 0;
    }

    pub fn process(&mut self, buffer: &mut AudioBuffer, ratio: f32, mix: f32) {
        self.process_with(buffer, ratio, mix, VocoderOptions::default());
    }

    pub fn process_with(
        &mut self,
        buffer: &mut AudioBuffer,
        ratio: f32,
        mix: f32,
        options: VocoderOptions,
    ) {
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
                self.frame(channels, ratio, options);
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

    fn frame(&mut self, channels: usize, ratio: f32, options: VocoderOptions) {
        let mut reset = Reset::None;
        for channel in 0..channels {
            for index in 0..FFT_SIZE {
                self.re[channel][index] = self.input[channel][index] * self.window[index];
                self.im[channel][index] = 0.0;
            }
            self.fft
                .forward(&mut self.re[channel], &mut self.im[channel]);
            self.analyse(channel, ratio);
            if channel == 0 {
                let flux = self.spectral_flux();
                let detected = self.is_transient(flux, options.detector);
                reset = match options.transients {
                    TransientMode::Smooth => Reset::None,
                    TransientMode::Mixed => {
                        if detected {
                            Reset::Outside
                        } else {
                            Reset::None
                        }
                    }
                    TransientMode::Crisp => {
                        if detected {
                            Reset::All
                        } else {
                            Reset::None
                        }
                    }
                };
            }
            self.synthesise(channel, options.phase, reset);
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

    fn spectral_flux(&mut self) -> f32 {
        let mut flux = 0.0f32;
        for bin in 0..BINS {
            let rise = self.magnitude[bin] - self.previous[bin];
            if rise > 0.0 {
                flux += rise;
            }
            self.previous[bin] = self.magnitude[bin];
        }
        flux
    }

    fn is_transient(&mut self, flux: f32, detector: TransientDetector) -> bool {
        let average = self.flux.max(1.0e-6);
        let detected = flux > 1.0e-3 && flux > average * detector.sensitivity();
        self.flux += (flux - self.flux) * 0.25;
        detected
    }

    fn lock(&self, bin: usize) -> usize {
        let mut reference = bin;
        if bin > 0 && self.magnitude[bin - 1] > self.magnitude[reference] {
            reference = bin - 1;
        }
        if bin + 1 < BINS && self.magnitude[bin + 1] > self.magnitude[reference] {
            reference = bin + 1;
        }
        reference
    }

    fn synthesise(&mut self, channel: usize, mode: PhaseMode, reset: Reset) {
        for bin in 0..BINS {
            if self.magnitude[bin] <= 0.0 {
                self.re[channel][bin] = 0.0;
                self.im[channel][bin] = 0.0;
                continue;
            }
            let reference = match mode {
                PhaseMode::Independent => bin,
                PhaseMode::Laminar => self.lock(bin),
            };
            let frequency = self.frequency[reference];
            let outside = frequency < RESET_LOW_BIN || frequency > RESET_HIGH_BIN;
            let reset_bin = match reset {
                Reset::All => true,
                Reset::Outside => outside,
                Reset::None => false,
            };
            if reset_bin {
                self.synthesis[channel][bin] = self.analysis[channel][bin];
            } else {
                let advance = TAU * HOP as f32 * frequency / FFT_SIZE as f32;
                self.synthesis[channel][bin] = wrap(self.synthesis[channel][bin] + advance);
            }
            let phase = self.synthesis[channel][bin];
            self.re[channel][bin] = self.magnitude[bin] * phase.cos();
            self.im[channel][bin] = self.magnitude[bin] * phase.sin();
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
