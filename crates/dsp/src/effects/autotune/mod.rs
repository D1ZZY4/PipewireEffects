use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit};

use crate::effects::vocoder::{HOP, Vocoder};
use crate::knob::Knob;

const HISTORY: usize = 1_024;
const MAX_FRAMES: usize = 8_192;
const MIN_CORRELATION: f32 = 0.5;
const MIN_FREQUENCY: f32 = 20.0;
const CHANNELS: usize = 2;

const SCALE_LABELS: [&str; 6] = [
    mark("Chromatic"),
    mark("Major"),
    mark("Minor"),
    mark("Harmonic minor"),
    mark("Major pentatonic"),
    mark("Minor pentatonic"),
];

const SCALES: [&[i32]; 6] = [
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    &[0, 2, 4, 5, 7, 9, 11],
    &[0, 2, 3, 5, 7, 8, 10],
    &[0, 2, 3, 5, 7, 8, 11],
    &[0, 2, 4, 7, 9],
    &[0, 3, 5, 7, 10],
];

const NOTE_LABELS: [&str; 12] = [
    mark("C"),
    mark("C sharp"),
    mark("D"),
    mark("D sharp"),
    mark("E"),
    mark("F"),
    mark("F sharp"),
    mark("G"),
    mark("G sharp"),
    mark("A"),
    mark("A sharp"),
    mark("B"),
];

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
        ParamSpec::boolean("fast_mode", "Fast correction", false).group("Correction"),
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
        ParamSpec::float("filter", "Detection filter", ParamUnit::None)
            .range(0.02, 0.5, 0.02)
            .step(0.01)
            .group("Detection")
            .build(),
        ParamSpec::choice("scale", "Scale", &SCALE_LABELS, 0).group("Scale"),
        ParamSpec::choice("root", "Root note", &NOTE_LABELS, 0).group("Scale"),
        ParamSpec::float("bend_range", "Bend range", ParamUnit::Semitones)
            .range(0.0, 7.0, 2.0)
            .step(0.1)
            .group("Scale")
            .build(),
        ParamSpec::float("offset", "Target offset", ParamUnit::Semitones)
            .range(-2.0, 2.0, 0.0)
            .step(0.1)
            .group("Scale")
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
    filter: Knob,
    scale: Knob,
    root: Knob,
    bend_range: Knob,
    offset: Knob,
    fast_mode: Knob,
    mix: Knob,
    history: [f32; HISTORY],
    scratch: AudioBuffer,
    ratio: f32,
    smoothed: f32,
    detecting: bool,
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
            filter: Knob::new(&params, "filter", 0.02),
            scale: Knob::new(&params, "scale", 0.0),
            root: Knob::new(&params, "root", 0.0),
            bend_range: Knob::new(&params, "bend_range", 2.0),
            offset: Knob::new(&params, "offset", 0.0),
            fast_mode: Knob::new(&params, "fast_mode", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            history: [0.0; HISTORY],
            scratch: AudioBuffer::new(CHANNELS, MAX_FRAMES),
            ratio: 1.0,
            smoothed: 0.0,
            detecting: false,
            rate: 48_000.0,
        }
    }

    fn allowed(&self, note: i32) -> bool {
        let scale = self.scale.index().min(SCALES.len() - 1);
        let root = (self.root.index() % 12) as i32;
        let relative = (note.rem_euclid(12) - root).rem_euclid(12);
        SCALES[scale].contains(&relative)
    }

    fn quantise(&self, midi: f32) -> f32 {
        let centre = midi.round() as i32;
        let mut best = midi;
        let mut best_distance = f32::MAX;
        for delta in -6..=6 {
            let candidate = centre + delta;
            if self.allowed(candidate) {
                let distance = (candidate as f32 - midi).abs();
                if distance < best_distance {
                    best_distance = distance;
                    best = candidate as f32;
                }
            }
        }
        best
    }

    fn detect(&self) -> Option<f32> {
        let mean = self.history.iter().sum::<f32>() / HISTORY as f32;
        let lowest = self.min_frequency.value().max(MIN_FREQUENCY);
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

    fn correction_ratio(&mut self, detected: f32, offset: f32, bend: f32) -> f32 {
        let reference = self.reference.value().max(1.0);
        let detected = detected.max(1.0);
        let midi = 69.0 + 12.0 * (detected / reference).log2();
        let smoothing = ((self.filter.value() - 0.02) / 0.48).clamp(0.0, 1.0);
        let coefficient = 1.0 - smoothing * 0.9;
        if self.detecting {
            self.smoothed += (midi - self.smoothed) * coefficient;
        } else {
            self.smoothed = midi;
            self.detecting = true;
        }
        let target = self.quantise(self.smoothed) + offset;
        let interval = (target - midi).clamp(-bend, bend);
        let corrected = midi + interval;
        let target_frequency = reference * ((corrected - 69.0) / 12.0).exp2();
        let ratio = (target_frequency / detected).clamp(0.5, 2.0);
        let strength = (self.strength.value() / 100.0).clamp(0.0, 1.0);
        1.0 + (ratio - 1.0) * strength
    }
}

fn push_history(history: &mut [f32; HISTORY], samples: &[f32]) {
    let frames = samples.len().min(HISTORY);
    if frames == 0 {
        return;
    }
    history.copy_within(frames.., 0);
    history[HISTORY - frames..].copy_from_slice(&samples[..frames]);
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
        self.smoothed = 0.0;
        self.detecting = false;
    }

    fn latency_frames(&self) -> usize {
        crate::effects::vocoder::LATENCY_FRAMES
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let frames = buffer.frames();
        let channels = buffer.channels().min(CHANNELS);
        if frames == 0 || channels == 0 {
            return;
        }
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let fast = self.fast_mode.flag();
        let offset = self.offset.value();
        let bend = self.bend_range.value().clamp(0.0, 7.0);
        let chunk = if fast { HOP } else { MAX_FRAMES };
        let mut position = 0;
        while position < frames {
            let take = chunk.min(frames - position);
            if self.scratch.configure(channels, take).is_err() {
                break;
            }
            for channel in 0..channels {
                self.scratch
                    .channel_mut(channel)
                    .copy_from_slice(&buffer.channel(channel)[position..position + take]);
            }
            if let Some(detected) = self.detect() {
                let wanted = self.correction_ratio(detected, offset, bend);
                if fast {
                    self.ratio = wanted;
                } else {
                    let block_ms = take as f32 / self.rate.max(1.0) * 1000.0;
                    let alpha = 1.0 - (-block_ms / self.speed.value().max(1.0)).exp();
                    self.ratio += (wanted - self.ratio) * alpha;
                }
                if !self.ratio.is_finite() {
                    self.ratio = 1.0;
                }
            }
            let ratio = self.ratio.clamp(0.5, 2.0);
            push_history(&mut self.history, self.scratch.channel(0));
            self.vocoder.process(&mut self.scratch, ratio, mix);
            for channel in 0..channels {
                buffer.channel_mut(channel)[position..position + take]
                    .copy_from_slice(self.scratch.channel(channel));
            }
            position += take;
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
