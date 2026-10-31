use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::knob::Knob;

mod estimator;

use estimator::DelayEstimator;

const MAX_FILTER_MS: f32 = 40.0;
const MOBILE_FILTER_MS: f32 = 20.0;
const AUTO_DELAY_MARGIN_MS: f32 = 2.0;
const EPSILON: f32 = 1.0e-9;
const HIGH_PASS_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
const DOUBLE_TALK_CORRELATION: f32 = 0.5;
const DOUBLE_TALK_FLOOR_DB: f32 = -55.0;
const AGC_TARGET_DB: f32 = -20.0;
const AGC_MAX_GAIN_DB: f32 = 20.0;
const AGC_MIN_GAIN_DB: f32 = -12.0;
const AGC_TIME_MS: f32 = 300.0;

const MODE_LABELS: [&str; 2] = [mark("Fixed"), mark("Mobile")];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("delay", "Echo delay", ParamUnit::Milliseconds)
            .range(0.0, 500.0, 0.0)
            .step(1.0)
            .group("Alignment")
            .build(),
        ParamSpec::boolean("auto_delay", "Automatic delay", false).group("Alignment"),
        ParamSpec::boolean("high_pass", "High-pass filter", false).group("Alignment"),
        ParamSpec::float("high_pass_cutoff", "High-pass cutoff", ParamUnit::Hertz)
            .range(20.0, 1_000.0, 80.0)
            .logarithmic()
            .group("Alignment")
            .build(),
        ParamSpec::choice("mode", "Mode", &MODE_LABELS, 0).group("Alignment"),
        ParamSpec::float("filter_length", "Filter length", ParamUnit::Milliseconds)
            .range(5.0, MAX_FILTER_MS, 10.0)
            .step(1.0)
            .group("Alignment")
            .build(),
        ParamSpec::float("adaptation", "Adaptation", ParamUnit::Percent)
            .range(0.0, 100.0, 50.0)
            .step(1.0)
            .group("Cancellation")
            .build(),
        ParamSpec::boolean("double_talk", "Double-talk detector", false).group("Cancellation"),
        ParamSpec::boolean("agc", "Automatic gain", false).group("Output"),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Cancellation")
            .build(),
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-30.0, 30.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    ]
}

pub struct EchoCanceller {
    delay: Knob,
    auto_delay: Knob,
    high_pass: Knob,
    high_pass_cutoff: Knob,
    mode: Knob,
    filter_length: Knob,
    adaptation: Knob,
    double_talk: Knob,
    agc: Knob,
    mix: Knob,
    output_gain: Knob,
    rate: u32,
    history: [Vec<f32>; 2],
    weights: [Vec<f32>; 2],
    scratch: Vec<f32>,
    mask: usize,
    write: usize,
    max_taps: usize,
    high_pass_near: [Biquad; 2],
    high_pass_far: [Biquad; 2],
    estimator: DelayEstimator,
    auto_delay_frames: f32,
    double_talk_active: bool,
    agc_gain_db: f32,
}

impl EchoCanceller {
    pub fn new(params: ParamSet) -> Self {
        let mut effect = Self {
            delay: Knob::new(&params, "delay", 0.0),
            auto_delay: Knob::new(&params, "auto_delay", 0.0),
            high_pass: Knob::new(&params, "high_pass", 0.0),
            high_pass_cutoff: Knob::new(&params, "high_pass_cutoff", 80.0),
            mode: Knob::new(&params, "mode", 0.0),
            filter_length: Knob::new(&params, "filter_length", 10.0),
            adaptation: Knob::new(&params, "adaptation", 50.0),
            double_talk: Knob::new(&params, "double_talk", 0.0),
            agc: Knob::new(&params, "agc", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            rate: 48_000,
            history: [Vec::new(), Vec::new()],
            weights: [Vec::new(), Vec::new()],
            scratch: Vec::new(),
            mask: 0,
            write: 0,
            max_taps: 0,
            high_pass_near: [Biquad::default(); 2],
            high_pass_far: [Biquad::default(); 2],
            estimator: DelayEstimator::new(48_000),
            auto_delay_frames: 0.0,
            double_talk_active: false,
            agc_gain_db: 0.0,
        };
        effect.allocate(48_000);
        effect
    }

    fn allocate(&mut self, rate: u32) {
        self.rate = rate.max(8_000);
        self.max_taps = (self.rate as f32 * MAX_FILTER_MS / 1000.0) as usize;
        let frames = (self.rate as usize / 2)
            .saturating_add(self.max_taps)
            .saturating_add(8_192)
            .next_power_of_two();
        for channel in 0..2 {
            self.history[channel] = vec![0.0; frames];
            self.weights[channel] = vec![0.0; self.max_taps];
        }
        self.mask = frames - 1;
        self.write = 0;
        self.scratch = vec![0.0; self.max_taps];
        self.estimator = DelayEstimator::new(self.rate);
        self.auto_delay_frames = 0.0;
        self.double_talk_active = false;
        self.agc_gain_db = 0.0;
    }

    fn mobile(&self) -> bool {
        self.mode.index() >= 1
    }

    fn filter_ms(&self) -> f32 {
        let requested = self.filter_length.value().clamp(5.0, MAX_FILTER_MS);
        if self.mobile() {
            requested.min(MOBILE_FILTER_MS)
        } else {
            requested
        }
    }

    fn effective_taps(&self) -> usize {
        let taps = ((self.filter_ms() / 1000.0) * self.rate as f32) as usize;
        taps.clamp(16, self.max_taps)
    }

    fn step_scale(&self) -> f32 {
        if self.mobile() { 2.0 } else { 1.0 }
    }

    fn aligned_delay(&mut self, taps: usize) -> usize {
        let manual = ((self.delay.value() / 1000.0) * self.rate as f32) as usize;
        let delay = if self.auto_delay.flag() {
            if self.estimator.has_estimate() {
                let raw = self.estimator.delay_frames() as f32;
                let coefficient = if self.mobile() { 0.5 } else { 0.25 };
                if self.auto_delay_frames <= 0.0 {
                    self.auto_delay_frames = raw;
                } else {
                    self.auto_delay_frames += (raw - self.auto_delay_frames) * coefficient;
                }
            }
            let margin = (self.rate as f32 * AUTO_DELAY_MARGIN_MS / 1000.0) as usize;
            (self.auto_delay_frames.max(0.0) as usize).saturating_sub(margin)
        } else {
            manual
        };
        delay.min(self.mask.saturating_sub(taps))
    }

    fn update_high_pass(&mut self) {
        if !self.high_pass.flag() {
            return;
        }
        let cutoff = self
            .high_pass_cutoff
            .value()
            .clamp(20.0, (self.rate as f32 * 0.45).max(20.0));
        let coefficients = Coefficients::design(
            FilterKind::HighPass,
            self.rate as f32,
            cutoff,
            HIGH_PASS_Q,
            0.0,
        );
        for channel in 0..2 {
            self.high_pass_near[channel].set_coefficients(coefficients);
            self.high_pass_far[channel].set_coefficients(coefficients);
        }
    }

    fn update_double_talk(&mut self, near_energy: f32, frames: usize) {
        if !self.double_talk.flag() || frames == 0 {
            self.double_talk_active = false;
            return;
        }
        let near_rms = (near_energy / frames as f32).sqrt();
        let near_db = math::gain_to_db(near_rms);
        self.double_talk_active = self.estimator.has_estimate()
            && self.estimator.correlation() < DOUBLE_TALK_CORRELATION
            && near_db > DOUBLE_TALK_FLOOR_DB;
    }

    fn update_agc(&mut self, energy: f32, count: usize, frames: usize) {
        if !self.agc.flag() || count == 0 || frames == 0 {
            if !self.agc.flag() {
                self.agc_gain_db = 0.0;
            }
            return;
        }
        let rms = (energy / count as f32).sqrt();
        let measured = math::gain_to_db(rms);
        if measured <= -90.0 {
            return;
        }
        let wanted = (AGC_TARGET_DB - measured).clamp(AGC_MIN_GAIN_DB, AGC_MAX_GAIN_DB);
        let block_ms = frames as f32 / self.rate.max(1) as f32 * 1000.0;
        let coefficient = 1.0 - (-block_ms / AGC_TIME_MS).exp();
        self.agc_gain_db += (wanted - self.agc_gain_db) * coefficient;
    }
}

impl Effect for EchoCanceller {
    fn id(&self) -> &'static str {
        "echo_canceller"
    }

    fn prepare(&mut self, format: AudioFormat) {
        if format.rate() != self.rate {
            self.allocate(format.rate());
        } else {
            self.reset();
        }
    }

    fn reset(&mut self) {
        for channel in 0..2 {
            self.history[channel].fill(0.0);
            self.weights[channel].fill(0.0);
            self.high_pass_near[channel].reset();
            self.high_pass_far[channel].reset();
        }
        self.write = 0;
        self.estimator.reset();
        self.auto_delay_frames = 0.0;
        self.double_talk_active = false;
        self.agc_gain_db = 0.0;
    }

    fn process_with_reference(&mut self, buffer: &mut AudioBuffer, reference: &[f32]) {
        let frames = buffer.frames();
        let channels = buffer.channels().min(2);
        if channels != 2 || reference.len() < frames * 2 {
            return;
        }
        let taps = self.effective_taps();
        let delay = self.aligned_delay(taps);
        let adaptation = (self.adaptation.value() / 100.0).clamp(0.0, 1.0);
        let mu = (0.5 * adaptation * self.step_scale()).min(1.0);
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let output = math::db_to_gain(self.output_gain.value());
        let high_pass = self.high_pass.flag();
        let agc = self.agc.flag();
        let agc_gain = math::db_to_gain(self.agc_gain_db);
        let freeze = self.double_talk.flag() && self.double_talk_active;
        let mask = self.mask;
        self.update_high_pass();
        let mut near_energy = 0.0f32;
        let mut wet_energy = 0.0f32;
        let mut wet_count = 0usize;
        for frame in 0..frames {
            for channel in 0..channels {
                let far_raw = reference[frame * 2 + channel];
                let near_raw = buffer.channel(channel)[frame];
                let far = if high_pass {
                    self.high_pass_far[channel].process(far_raw)
                } else {
                    far_raw
                };
                let near = if high_pass {
                    self.high_pass_near[channel].process(near_raw)
                } else {
                    near_raw
                };
                if channel == 0 {
                    self.estimator.push(far, near);
                    near_energy += near_raw * near_raw;
                }
                self.history[channel][self.write] = far;
                let start = (self.write + mask + 1 - delay) & mask;
                {
                    let history = &self.history[channel];
                    for tap in 0..taps {
                        self.scratch[tap] = history[(start + mask + 1 - tap) & mask];
                    }
                }
                let samples = &self.scratch[..taps];
                let weights = &mut self.weights[channel][..taps];
                let mut estimate = 0.0f32;
                let mut energy = EPSILON;
                for (weight, sample) in weights.iter().zip(samples.iter()) {
                    estimate += weight * sample;
                    energy += sample * sample;
                }
                let error = near - estimate;
                if mu > 0.0 && !freeze {
                    let step = mu * error / energy;
                    for (weight, sample) in weights.iter_mut().zip(samples.iter()) {
                        *weight += step * sample;
                    }
                }
                let wet = error * mix * if agc { agc_gain } else { 1.0 };
                wet_energy += wet * wet;
                wet_count += 1;
                let mixed = near_raw * (1.0 - mix) + wet;
                buffer.channel_mut(channel)[frame] = math::sanitize(mixed * output);
            }
            self.write = (self.write + 1) & mask;
        }
        self.update_double_talk(near_energy, frames);
        self.update_agc(wet_energy, wet_count, frames);
    }

    fn process(&mut self, _buffer: &mut AudioBuffer) {}
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/estimator.rs"]
mod estimator_tests;
