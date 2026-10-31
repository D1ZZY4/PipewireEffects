use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit};

use crate::effects::vocoder::Vocoder;
use crate::knob::Knob;

pub use crate::effects::vocoder::LATENCY_FRAMES;

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
    mix: Knob,
}

impl PitchShift {
    pub fn new(params: ParamSet) -> Self {
        Self {
            vocoder: Vocoder::new(),
            semitones: Knob::new(&params, "semitones", 0.0),
            cents: Knob::new(&params, "cents", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
        }
    }

    fn ratio(&self) -> f32 {
        ((self.semitones.value() + self.cents.value() / 100.0) / 12.0).exp2()
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
        self.vocoder.process(buffer, ratio, mix);
    }
}

#[cfg(test)]
mod tests;
