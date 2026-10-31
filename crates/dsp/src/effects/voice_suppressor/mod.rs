use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::fft::{Fft, hann, magnitude_db};
use crate::knob::Knob;

const FFT_SIZE: usize = 1024;
const HOP: usize = FFT_SIZE / 4;
const BINS: usize = FFT_SIZE / 2 + 1;
const WINDOW_SUM: f32 = 1.5;
const KURTOSIS_EPSILON: f32 = 1.0e-12;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("start", "Start", ParamUnit::Hertz)
            .range(40.0, 20_000.0, 200.0)
            .logarithmic()
            .group("Range")
            .build(),
        ParamSpec::float("end", "End", ParamUnit::Hertz)
            .range(40.0, 20_000.0, 8_000.0)
            .logarithmic()
            .group("Range")
            .build(),
        ParamSpec::float("correlation", "Correlation", ParamUnit::None)
            .range(0.0, 1.0, 0.5)
            .step(0.01)
            .group("Detection")
            .build(),
        ParamSpec::float("phase", "Phase difference", ParamUnit::None)
            .range(0.0, 180.0, 30.0)
            .step(1.0)
            .group("Detection")
            .build(),
        ParamSpec::float("min_kurtosis", "Minimum kurtosis", ParamUnit::None)
            .range(0.0, 100.0, 1.0)
            .step(0.01)
            .group("Detection")
            .build(),
        ParamSpec::boolean("inverted", "Inverted mode", false).group("Detection"),
        ParamSpec::float(
            "max_if",
            "Maximum instantaneous frequency",
            ParamUnit::Hertz,
        )
        .range(50.0, 20_000.0, 12_000.0)
        .logarithmic()
        .group("Detection")
        .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct VoiceSuppressor {
    fft: Fft,
    window: Vec<f32>,
    start: Knob,
    end: Knob,
    correlation: Knob,
    phase: Knob,
    min_kurtosis: Knob,
    inverted: Knob,
    max_if: Knob,
    mix: Knob,
    previous: [f32; BINS],
    instantaneous: [f32; BINS],
    magnitude: [[f32; BINS]; MAX_CHANNELS],
    rate: f32,
    input: [[f32; FFT_SIZE]; MAX_CHANNELS],
    output: [[f32; FFT_SIZE]; MAX_CHANNELS],
    re: [[f32; FFT_SIZE]; MAX_CHANNELS],
    im: [[f32; FFT_SIZE]; MAX_CHANNELS],
}

impl VoiceSuppressor {
    pub fn new(params: ParamSet) -> Self {
        Self {
            fft: Fft::new(FFT_SIZE),
            window: hann(FFT_SIZE),
            start: Knob::new(&params, "start", 200.0),
            end: Knob::new(&params, "end", 8_000.0),
            correlation: Knob::new(&params, "correlation", 0.5),
            phase: Knob::new(&params, "phase", 30.0),
            min_kurtosis: Knob::new(&params, "min_kurtosis", 1.0),
            inverted: Knob::new(&params, "inverted", 0.0),
            max_if: Knob::new(&params, "max_if", 12_000.0),
            mix: Knob::new(&params, "mix", 100.0),
            previous: [0.0; BINS],
            instantaneous: [0.0; BINS],
            magnitude: [[0.0; BINS]; MAX_CHANNELS],
            rate: 48_000.0,
            input: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            output: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            re: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            im: [[0.0; FFT_SIZE]; MAX_CHANNELS],
        }
    }

    fn transform(&mut self, channels: usize) {
        let start = self.start.value();
        let end = self.end.value();
        let correlation = self.correlation.value();
        let phase_limit = self.phase.value().to_radians();
        let min_kurtosis = self.min_kurtosis.value();
        let max_if = self.max_if.value();
        let inverted = self.inverted.flag();
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let bin_hz = self.rate / FFT_SIZE as f32;
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
                self.magnitude[channel][bin] = self.re[channel][bin].hypot(self.im[channel][bin]);
            }
        }
        let scale = self.rate / (std::f32::consts::TAU * HOP as f32);
        for bin in 1..BINS {
            let phase = self.im[0][bin].atan2(self.re[0][bin]);
            let delta = wrap(phase - self.previous[bin]);
            self.previous[bin] = phase;
            let expected = std::f32::consts::TAU * bin as f32 * HOP as f32 / FFT_SIZE as f32;
            let deviation = wrap(delta - expected);
            self.instantaneous[bin] = (bin as f32 * bin_hz + deviation * scale).abs();
        }
        for bin in 1..BINS {
            let frequency = bin as f32 * bin_hz;
            if frequency < start || frequency > end {
                continue;
            }
            let (left_re, left_im) = if channels > 1 {
                (self.re[1][bin], self.im[1][bin])
            } else {
                (self.re[0][bin], self.im[0][bin])
            };
            let (right_re, right_im) = (self.re[0][bin], self.im[0][bin]);
            let left_power = left_re * left_re + left_im * left_im;
            let right_power = right_re * right_re + right_im * right_im;
            if magnitude_db(left_re, left_im).max(magnitude_db(right_re, right_im)) < -90.0 {
                continue;
            }
            let cross_re = left_re * right_re + left_im * right_im;
            let cross_im = left_im * right_re - left_re * right_im;
            let denominator = (left_power * right_power).sqrt();
            let coefficient = if denominator <= f32::MIN_POSITIVE {
                0.0
            } else {
                ((cross_re * cross_re + cross_im * cross_im).sqrt() / denominator).clamp(0.0, 1.0)
            };
            let difference = cross_im.atan2(cross_re).abs();
            let kurtosis = self.local_kurtosis(bin, channels);
            let eligible = self.instantaneous[bin] <= max_if && kurtosis >= min_kurtosis;
            let mask = if eligible {
                voice_mask(
                    coefficient,
                    difference,
                    kurtosis,
                    self.instantaneous[bin],
                    (correlation, phase_limit, min_kurtosis, max_if),
                )
            } else {
                0.0
            };
            let gain = if inverted {
                1.0 - mix * (1.0 - mask)
            } else {
                1.0 - mix * mask
            };
            if gain >= 1.0 {
                continue;
            }
            for channel in 0..channels {
                self.re[channel][bin] *= gain;
                self.im[channel][bin] *= gain;
            }
        }
        for channel in 0..channels {
            self.im[channel][0] = 0.0;
            self.im[channel][FFT_SIZE / 2] = 0.0;
            for bin in 1..BINS - 1 {
                self.re[channel][FFT_SIZE - bin] = self.re[channel][bin];
                self.im[channel][FFT_SIZE - bin] = -self.im[channel][bin];
            }
        }
    }

    fn local_kurtosis(&self, bin: usize, channels: usize) -> f32 {
        let left = spectral_kurtosis(&self.magnitude[0], bin);
        if channels > 1 {
            left.max(spectral_kurtosis(&self.magnitude[1], bin))
        } else {
            left
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

impl Effect for VoiceSuppressor {
    fn id(&self) -> &'static str {
        "voice_suppressor"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.input {
            channel.fill(0.0);
        }
        for channel in &mut self.output {
            channel.fill(0.0);
        }
        self.previous.fill(0.0);
        self.instantaneous.fill(0.0);
        for channel in &mut self.magnitude {
            channel.fill(0.0);
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
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
}

fn wrap(phase: f32) -> f32 {
    phase - std::f32::consts::TAU * (phase / std::f32::consts::TAU).round()
}

fn ratio_sigmoid(x: f32) -> f32 {
    0.5 * (1.0 + x / (1.0 + x.abs()))
}

fn voice_mask(
    correlation: f32,
    phase_diff: f32,
    kurtosis: f32,
    instantaneous: f32,
    thresholds: (f32, f32, f32, f32),
) -> f32 {
    let (correlation_threshold, phase_limit, minimum_kurtosis, maximum_frequency) = thresholds;
    let correlation_gain = ratio_sigmoid((correlation - correlation_threshold) / 0.08);
    let phase_gain = ratio_sigmoid((phase_limit - phase_diff) / 0.08);
    let kurtosis_width = 0.08 * minimum_kurtosis.max(0.5);
    let kurtosis_gain = ratio_sigmoid((kurtosis - minimum_kurtosis) / kurtosis_width);
    let frequency_width = 0.04 * maximum_frequency.max(1.0);
    let frequency_gain = ratio_sigmoid((maximum_frequency - instantaneous) / frequency_width);
    (correlation_gain * phase_gain * kurtosis_gain * frequency_gain).clamp(0.0, 1.0)
}

fn spectral_kurtosis(magnitudes: &[f32], bin: usize) -> f32 {
    let first = bin.saturating_sub(1);
    let last = (bin + 1).min(BINS - 1);
    let window = &magnitudes[first..=last];
    let count = window.len() as f32;
    let mean = window.iter().sum::<f32>() / count;
    let mut variance = 0.0;
    let mut fourth = 0.0;
    for magnitude in window {
        let delta = *magnitude - mean;
        variance += delta * delta;
        fourth += delta * delta * delta * delta;
    }
    variance /= count;
    fourth /= count;
    fourth / (variance * variance + KURTOSIS_EPSILON)
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
