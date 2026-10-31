use pwe_core::AudioBuffer;

use crate::fft::{Fft, hann};

pub const BANDS: usize = 48;

const FFT_SIZE: usize = 8192;
const STRIDE: u32 = 3;
const MIN_HZ: f32 = 20.0;
const MAX_HZ: f32 = 20_000.0;
pub const FLOOR_DB: f32 = -120.0;
pub const CEILING_DB: f32 = 6.0;

pub fn band_centre(index: usize) -> f32 {
    let ratio = MAX_HZ / MIN_HZ;
    MIN_HZ * ratio.powf((index as f32 + 0.5) / BANDS as f32)
}

pub struct SpectrumAnalyzer {
    fft: Fft,
    window: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    history: Vec<f32>,
    cursor: usize,
    filled: usize,
    tick: u32,
    power: Vec<f32>,
    cumulative: Vec<f32>,
    bands: Vec<(f32, f32)>,
    power_scale: f32,
    bin_width: f32,
    resolution_bandwidth: f32,
    values: [f32; BANDS],
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrumAnalyzer {
    pub fn new() -> Self {
        let mut analyzer = Self {
            fft: Fft::new(FFT_SIZE),
            window: hann(FFT_SIZE),
            re: vec![0.0; FFT_SIZE],
            im: vec![0.0; FFT_SIZE],
            history: vec![0.0; FFT_SIZE],
            cursor: 0,
            filled: 0,
            tick: 0,
            power: vec![0.0; FFT_SIZE / 2 + 1],
            cumulative: vec![0.0; FFT_SIZE / 2 + 1],
            bands: Vec::with_capacity(BANDS),
            power_scale: 1.0,
            bin_width: 1.0,
            resolution_bandwidth: 1.0,
            values: [FLOOR_DB; BANDS],
        };
        analyzer.prepare(48_000);
        analyzer
    }

    pub fn prepare(&mut self, rate: u32) {
        let rate = rate.max(8_000) as f32;
        let sum: f32 = self.window.iter().sum();
        let squares: f32 = self.window.iter().map(|value| value * value).sum();
        self.bin_width = rate / FFT_SIZE as f32;
        self.power_scale = 1.0 / (FFT_SIZE as f32 * squares);
        self.resolution_bandwidth = rate * squares / (sum * sum);
        let nyquist = 0.5 * rate;
        let ratio = MAX_HZ / MIN_HZ;
        self.bands.clear();
        for index in 0..BANDS {
            let low = MIN_HZ * ratio.powf(index as f32 / BANDS as f32);
            let high = MIN_HZ * ratio.powf((index + 1) as f32 / BANDS as f32);
            self.bands.push((low.min(nyquist), high.min(nyquist)));
        }
        self.reset();
    }

    fn reset(&mut self) {
        self.history.fill(0.0);
        self.cursor = 0;
        self.filled = 0;
        self.tick = 0;
        self.values = [FLOOR_DB; BANDS];
    }

    pub fn push(&mut self, buffer: &AudioBuffer) -> Option<[f32; BANDS]> {
        self.accumulate(buffer);
        self.tick += 1;
        if self.tick < STRIDE || self.filled < FFT_SIZE {
            return None;
        }
        self.tick = 0;
        self.transform();
        Some(self.values)
    }

    fn accumulate(&mut self, buffer: &AudioBuffer) {
        let frames = buffer.frames();
        if buffer.channels() < 2 {
            for index in 0..frames {
                self.write(buffer.channel(0)[index]);
            }
            return;
        }
        let left = buffer.channel(0);
        let right = buffer.channel(1);
        let frames = frames.min(left.len()).min(right.len());
        for index in 0..frames {
            self.write(0.5 * (left[index] + right[index]));
        }
    }

    fn write(&mut self, value: f32) {
        self.history[self.cursor] = value;
        self.cursor = (self.cursor + 1) % FFT_SIZE;
        self.filled = (self.filled + 1).min(FFT_SIZE);
    }

    fn transform(&mut self) {
        for index in 0..FFT_SIZE {
            let position = (self.cursor + index) % FFT_SIZE;
            self.re[index] = self.history[position] * self.window[index];
            self.im[index] = 0.0;
        }
        self.fft.forward(&mut self.re, &mut self.im);
        let bins = self.power.len();
        for bin in 0..bins {
            self.power[bin] =
                (self.re[bin] * self.re[bin] + self.im[bin] * self.im[bin]) * self.power_scale;
        }
        self.cumulative[0] = 0.0;
        for bin in 1..bins {
            self.cumulative[bin] =
                self.cumulative[bin - 1] + 0.5 * (self.power[bin - 1] + self.power[bin]);
        }
        for (index, (low, high)) in self.bands.iter().enumerate() {
            self.values[index] = self.band_db(*low, *high);
        }
    }

    fn band_db(&self, low: f32, high: f32) -> f32 {
        let width = high - low;
        if width <= 0.0 {
            return FLOOR_DB;
        }
        let mut power = self.integral_to(high) - self.integral_to(low);
        if width < self.resolution_bandwidth {
            power *= self.resolution_bandwidth / width;
        }
        if power <= 0.0 {
            return FLOOR_DB;
        }
        (10.0 * (4.0 * power).log10()).clamp(FLOOR_DB, CEILING_DB)
    }

    fn integral_to(&self, frequency: f32) -> f32 {
        let last = self.power.len() - 1;
        let position = (frequency / self.bin_width).clamp(0.0, last as f32);
        let bin = (position as usize).min(last - 1);
        let offset = position - bin as f32;
        let left = self.power[bin];
        let right = self.power[bin + 1];
        self.cumulative[bin] + left * offset + 0.5 * (right - left) * offset * offset
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
