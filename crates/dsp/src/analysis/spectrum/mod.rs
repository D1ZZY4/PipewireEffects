use pwe_core::AudioBuffer;

use crate::fft::Fft;

pub const BANDS: usize = 48;

pub const FFT_SIZES: [usize; 4] = [2_048, 4_096, 8_192, 16_384];
pub const WINDOWS: [&str; 3] = ["Hann", "Hamming", "Blackman"];

const DEFAULT_SIZE: u8 = 2;
const DEFAULT_WINDOW: u8 = 0;
const MAX_SIZE: usize = 16_384;
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
    ffts: Vec<Fft>,
    windows: Vec<Vec<Vec<f32>>>,
    size_index: u8,
    window_index: u8,
    rate: u32,
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
        Self::with_options(DEFAULT_SIZE, DEFAULT_WINDOW)
    }

    pub fn with_options(size_index: u8, window_index: u8) -> Self {
        let ffts = FFT_SIZES.iter().map(|size| Fft::new(*size)).collect();
        let windows = FFT_SIZES
            .iter()
            .map(|size| {
                WINDOWS
                    .iter()
                    .enumerate()
                    .map(|(kind, _)| window(kind, *size))
                    .collect()
            })
            .collect();
        let mut analyzer = Self {
            ffts,
            windows,
            size_index: size_index.min(FFT_SIZES.len() as u8 - 1),
            window_index: window_index.min(WINDOWS.len() as u8 - 1),
            rate: 48_000,
            re: vec![0.0; MAX_SIZE],
            im: vec![0.0; MAX_SIZE],
            history: vec![0.0; MAX_SIZE],
            cursor: 0,
            filled: 0,
            tick: 0,
            power: vec![0.0; MAX_SIZE / 2 + 1],
            cumulative: vec![0.0; MAX_SIZE / 2 + 1],
            bands: Vec::with_capacity(BANDS),
            power_scale: 1.0,
            bin_width: 1.0,
            resolution_bandwidth: 1.0,
            values: [FLOOR_DB; BANDS],
        };
        analyzer.prepare(48_000);
        analyzer
    }

    pub fn size(&self) -> usize {
        FFT_SIZES[self.size_index as usize]
    }

    pub fn window_name(&self) -> &'static str {
        WINDOWS[self.window_index as usize]
    }

    pub fn set_options(&mut self, size_index: u8, window_index: u8) {
        let size_index = size_index.min(FFT_SIZES.len() as u8 - 1);
        let window_index = window_index.min(WINDOWS.len() as u8 - 1);
        if size_index == self.size_index && window_index == self.window_index {
            return;
        }
        self.size_index = size_index;
        self.window_index = window_index;
        self.prepare(self.rate);
    }

    pub fn prepare(&mut self, rate: u32) {
        self.rate = rate.max(8_000);
        let rate = self.rate as f32;
        let size = self.size();
        let window = &self.windows[self.size_index as usize][self.window_index as usize];
        let sum: f32 = window.iter().sum();
        let squares: f32 = window.iter().map(|value| value * value).sum();
        self.bin_width = rate / size as f32;
        self.power_scale = 1.0 / (size as f32 * squares);
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
        if self.tick < STRIDE || self.filled < self.size() {
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
        self.cursor = (self.cursor + 1) % MAX_SIZE;
        self.filled = (self.filled + 1).min(MAX_SIZE);
    }

    fn transform(&mut self) {
        let size = self.size();
        let window = self.windows[self.size_index as usize][self.window_index as usize].as_slice();
        let base = (self.cursor + MAX_SIZE - size) % MAX_SIZE;
        for (index, (re, weight)) in self.re.iter_mut().zip(window).enumerate().take(size) {
            let position = (base + index) % MAX_SIZE;
            *re = self.history[position] * weight;
            self.im[index] = 0.0;
        }
        self.ffts[self.size_index as usize].forward(&mut self.re, &mut self.im);
        let bins = size / 2 + 1;
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
        let last = self.size() / 2;
        let position = (frequency / self.bin_width).clamp(0.0, last as f32);
        let bin = (position as usize).min(last - 1);
        let offset = position - bin as f32;
        let left = self.power[bin];
        let right = self.power[bin + 1];
        self.cumulative[bin] + left * offset + 0.5 * (right - left) * offset * offset
    }
}

fn window(kind: usize, size: usize) -> Vec<f32> {
    (0..size)
        .map(|index| {
            let angle = std::f32::consts::TAU * index as f32 / size.max(1) as f32;
            match kind {
                1 => 0.54 - 0.46 * angle.cos(),
                2 => 0.42 - 0.5 * angle.cos() + 0.08 * (2.0 * angle).cos(),
                _ => 0.5 - 0.5 * angle.cos(),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
