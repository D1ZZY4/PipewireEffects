use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::dynamics::{ballistics, soft_knee_reduction_db};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const MODES: [&str; 2] = [mark("Split"), mark("Wideband")];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("frequency", "Sibilance", ParamUnit::Hertz)
            .range(2_000.0, 12_000.0, 6_000.0)
            .logarithmic()
            .group("Detector")
            .build(),
        ParamSpec::float("threshold", "Threshold", ParamUnit::Decibel)
            .range(-40.0, 0.0, -20.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::float("ratio", "Ratio", ParamUnit::Ratio)
            .range(1.0, 10.0, 4.0)
            .step(0.1)
            .group("Detector")
            .build(),
        ParamSpec::choice("mode", "Mode", &MODES, 0).group("Detector"),
        ParamSpec::float("attack", "Attack", ParamUnit::Milliseconds)
            .range(0.1, 50.0, 5.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(10.0, 500.0, 100.0)
            .logarithmic()
            .group("Timing")
            .build(),
    ]
}

pub struct DeEsser {
    frequency: Knob,
    threshold: Knob,
    ratio: Knob,
    mode: Knob,
    attack: Knob,
    release: Knob,
    band_pass: [Biquad; MAX_CHANNELS],
    envelope: f32,
    attack_coefficient: f32,
    release_coefficient: f32,
    rate: u32,
    applied: [f32; 3],
}

impl DeEsser {
    pub fn new(params: ParamSet) -> Self {
        Self {
            frequency: Knob::new(&params, "frequency", 6_000.0),
            threshold: Knob::new(&params, "threshold", -20.0),
            ratio: Knob::new(&params, "ratio", 4.0),
            mode: Knob::new(&params, "mode", 0.0),
            attack: Knob::new(&params, "attack", 5.0),
            release: Knob::new(&params, "release", 100.0),
            band_pass: [Biquad::default(); MAX_CHANNELS],
            envelope: 0.0,
            attack_coefficient: 0.0,
            release_coefficient: 0.0,
            rate: 48_000,
            applied: [f32::NAN; 3],
        }
    }

    fn signature(&self) -> [f32; 3] {
        [
            self.frequency.value(),
            self.attack.value(),
            self.release.value(),
        ]
    }

    fn refresh(&mut self, signature: [f32; 3]) {
        let coefficients = Coefficients::design(
            FilterKind::BandPass,
            self.rate as f32,
            signature[0],
            1.5,
            0.0,
        );
        for biquad in &mut self.band_pass {
            biquad.set_coefficients(coefficients);
        }
        let (attack, release) = ballistics(signature[1], signature[2], self.rate);
        self.attack_coefficient = attack;
        self.release_coefficient = release;
        self.applied = signature;
    }
}

impl Effect for DeEsser {
    fn id(&self) -> &'static str {
        "deesser"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.applied = [f32::NAN; 3];
        self.reset();
    }

    fn reset(&mut self) {
        for biquad in &mut self.band_pass {
            biquad.reset();
        }
        self.envelope = 0.0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let signature = self.signature();
        if signature != self.applied {
            self.refresh(signature);
        }
        let threshold = self.threshold.value();
        let ratio = self.ratio.value();
        let attack = self.attack_coefficient;
        let release = self.release_coefficient;
        let wideband = self.mode.index() == 1;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for frame in 0..frames {
            let mut detection = 0.0f32;
            let mut band = [0.0f32; MAX_CHANNELS];
            for (channel, slot) in band.iter_mut().enumerate().take(channels) {
                let value = self.band_pass[channel].process(buffer.channel_mut(channel)[frame]);
                *slot = value;
                detection = detection.max(value.abs());
            }
            let target = detection;
            let coefficient = if target > self.envelope {
                attack
            } else {
                release
            };
            self.envelope += (target - self.envelope) * coefficient;
            if !self.envelope.is_finite() {
                self.envelope = 0.0;
            }
            let reduction =
                soft_knee_reduction_db(math::gain_to_db(self.envelope), threshold, 0.0, ratio);
            let gain = math::db_to_gain(reduction);
            for (channel, &banded) in band.iter().enumerate().take(channels) {
                let plan = buffer.channel_mut(channel);
                let dry = plan[frame];
                let value = if wideband {
                    dry * gain
                } else {
                    (dry - banded) + banded * gain
                };
                plan[frame] = math::sanitize(value);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
