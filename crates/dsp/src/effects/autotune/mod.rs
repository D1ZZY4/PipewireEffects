use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit};

use crate::effects::vocoder::Vocoder;
use crate::knob::Knob;

const HISTORY: usize = 1_024;
const MIN_CORRELATION: f32 = 0.5;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("speed", "Speed", ParamUnit::Milliseconds)
            .range(5.0, 200.0, 30.0)
            .logarithmic()
            .group("Correction")
            .build(),
        ParamSpec::float("strength", "Strength", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Correction")
            .build(),
        ParamSpec::float("min_frequency", "Lowest pitch", ParamUnit::Hertz)
            .range(50.0, 500.0, 80.0)
            .logarithmic()
            .group("Detection")
            .build(),
        ParamSpec::float("max_frequency", "Highest pitch", ParamUnit::Hertz)
            .range(200.0, 2_000.0, 1_000.0)
            .logarithmic()
            .group("Detection")
            .build(),
        ParamSpec::float("reference", "Reference pitch", ParamUnit::Hertz)
            .range(420.0, 460.0, 440.0)
            .step(0.5)
            .group("Detection")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct Autotune {
    vocoder: Vocoder,
    speed: Knob,
    strength: Knob,
    min_frequency: Knob,
    max_frequency: Knob,
    reference: Knob,
    mix: Knob,
    history: [f32; HISTORY],
    ratio: f32,
    rate: f32,
}

impl Autotune {
    pub fn new(params: ParamSet) -> Self {
        Self {
            vocoder: Vocoder::new(),
            speed: Knob::new(&params, "speed", 30.0),
            strength: Knob::new(&params, "strength", 100.0),
            min_frequency: Knob::new(&params, "min_frequency", 80.0),
            max_frequency: Knob::new(&params, "max_frequency", 1_000.0),
            reference: Knob::new(&params, "reference", 440.0),
            mix: Knob::new(&params, "mix", 100.0),
            history: [0.0; HISTORY],
            ratio: 1.0,
            rate: 48_000.0,
        }
    }

    fn detect(&self) -> Option<f32> {
        let mean = self.history.iter().sum::<f32>() / HISTORY as f32;
        let lowest = self.min_frequency.value().max(20.0);
        let highest = self.max_frequency.value().max(lowest + 1.0);
        let first = (self.rate / highest).floor() as usize;
        let last = (self.rate / lowest).ceil() as usize;
        let last = last.min(HISTORY - 2);
        if first < 2 || first >= last {
            return None;
        }
        let mut best_lag = 0usize;
        let mut best = 0.0f32;
        for lag in first..=last {
            let mut product = 0.0f32;
            let mut left = 0.0f32;
            let mut right = 0.0f32;
            for index in 0..HISTORY - lag {
                let a = self.history[index] - mean;
                let b = self.history[index + lag] - mean;
                product += a * b;
                left += a * a;
                right += b * b;
            }
            let denominator = (left * right).sqrt();
            if denominator <= f32::MIN_POSITIVE {
                continue;
            }
            let correlation = product / denominator;
            if correlation > best {
                best = correlation;
                best_lag = lag;
            }
        }
        if best_lag == 0 || best < MIN_CORRELATION {
            return None;
        }
        Some(self.rate / best_lag as f32)
    }

    fn correction_ratio(&self, detected: f32) -> f32 {
        let reference = self.reference.value().max(1.0);
        let semitones = 69.0 + 12.0 * (detected / reference).log2();
        let target = reference * ((semitones.round() - 69.0) / 12.0).exp2();
        let ratio = (target / detected).clamp(0.5, 2.0);
        let strength = (self.strength.value() / 100.0).clamp(0.0, 1.0);
        1.0 + (ratio - 1.0) * strength
    }

    fn update_history(&mut self, buffer: &AudioBuffer) {
        let frames = buffer.frames().min(HISTORY);
        if frames == 0 {
            return;
        }
        self.history.copy_within(frames.., 0);
        let source = buffer.channel(0);
        self.history[HISTORY - frames..].copy_from_slice(&source[..frames]);
    }
}

impl Effect for Autotune {
    fn id(&self) -> &'static str {
        "autotune"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(8_000) as f32;
        self.reset();
    }

    fn reset(&mut self) {
        self.vocoder.reset();
        self.history.fill(0.0);
        self.ratio = 1.0;
    }

    fn latency_frames(&self) -> usize {
        crate::effects::vocoder::LATENCY_FRAMES
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        if let Some(detected) = self.detect() {
            let wanted = self.correction_ratio(detected);
            let block_ms = buffer.frames() as f32 / self.rate.max(1.0) * 1000.0;
            let alpha = 1.0 - (-block_ms / self.speed.value().max(1.0)).exp();
            self.ratio += (wanted - self.ratio) * alpha;
            if !self.ratio.is_finite() {
                self.ratio = 1.0;
            }
        }
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let ratio = self.ratio.clamp(0.5, 2.0);
        self.update_history(buffer);
        self.vocoder.process(buffer, ratio, mix);
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
