use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit};

use crate::effects::vocoder::{
    PhaseMode, TransientDetector, TransientMode, Vocoder, VocoderOptions,
};
use crate::knob::Knob;

pub use crate::effects::vocoder::LATENCY_FRAMES;

const TRANSIENT_LABELS: [&str; 3] = [mark("Smooth"), mark("Mixed"), mark("Crisp")];
const DETECTOR_LABELS: [&str; 3] = [mark("Compound"), mark("Percussive"), mark("Soft")];
const PHASE_LABELS: [&str; 2] = [mark("Independent"), mark("Laminar")];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("semitones", "Semitones", ParamUnit::Semitones)
            .range(-24.0, 24.0, 0.0)
            .step(1.0)
            .group("Shift")
            .build(),
        ParamSpec::float("cents", "Cents", ParamUnit::Cents)
            .range(-100.0, 100.0, 0.0)
            .step(1.0)
            .group("Shift")
            .build(),
        ParamSpec::choice("transients", "Transients", &TRANSIENT_LABELS, 0).group("Quality"),
        ParamSpec::choice("detector", "Transient detector", &DETECTOR_LABELS, 0).group("Quality"),
        ParamSpec::choice("phase", "Phase", &PHASE_LABELS, 0).group("Quality"),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct PitchShift {
    vocoder: Vocoder,
    semitones: Knob,
    cents: Knob,
    transients: Knob,
    detector: Knob,
    phase: Knob,
    mix: Knob,
}

impl PitchShift {
    pub fn new(params: ParamSet) -> Self {
        Self {
            vocoder: Vocoder::new(),
            semitones: Knob::new(&params, "semitones", 0.0),
            cents: Knob::new(&params, "cents", 0.0),
            transients: Knob::new(&params, "transients", 0.0),
            detector: Knob::new(&params, "detector", 0.0),
            phase: Knob::new(&params, "phase", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
        }
    }

    fn ratio(&self) -> f32 {
        ((self.semitones.value() + self.cents.value() / 100.0) / 12.0).exp2()
    }

    fn options(&self) -> VocoderOptions {
        VocoderOptions {
            phase: PhaseMode::from_index(self.phase.index()),
            transients: TransientMode::from_index(self.transients.index()),
            detector: TransientDetector::from_index(self.detector.index()),
        }
    }
}

impl Effect for PitchShift {
    fn id(&self) -> &'static str {
        "pitch_shift"
    }

    fn prepare(&mut self, _format: AudioFormat) {
        self.reset();
    }

    fn reset(&mut self) {
        self.vocoder.reset();
    }

    fn latency_frames(&self) -> usize {
        LATENCY_FRAMES
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let ratio = self.ratio();
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        self.vocoder
            .process_with(buffer, ratio, mix, self.options());
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
