use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::dynamics::{ballistics, soft_knee_reduction_db};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const MODES: [&str; 2] = ["Split", "Wideband"];

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
    dirty: bool,
}

impl DeEsser {
    pub fn new(params: ParamSet) -> Self {
        let mut effect = Self {
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
            dirty: true,
        };
        effect.refresh();
        effect
    }

    fn refresh(&mut self) {
        let coefficients = Coefficients::design(
            FilterKind::BandPass,
            self.rate as f32,
            self.frequency.value(),
            1.5,
            0.0,
        );
        for biquad in &mut self.band_pass {
            biquad.set_coefficients(coefficients);
        }
        let (attack, release) = ballistics(self.attack.value(), self.release.value(), self.rate);
        self.attack_coefficient = attack;
        self.release_coefficient = release;
    }
}

impl Effect for DeEsser {
    fn id(&self) -> &'static str {
        "deesser"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for biquad in &mut self.band_pass {
            biquad.reset();
        }
        self.envelope = 0.0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        if self.dirty {
            self.dirty = false;
            self.refresh();
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
mod tests {
    use super::*;

    fn tone(frequency: f32, amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample =
                    amplitude * (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    #[test]
    fn sibilance_is_reduced() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -30.0).unwrap();
        params.set_f32("ratio", 8.0).unwrap();
        let mut effect = DeEsser::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..256 {
            let mut buffer = tone(6_000.0, 0.7, 1024);
            effect.process(&mut buffer);
        }
        let mut buffer = tone(6_000.0, 0.7, 1024);
        effect.process(&mut buffer);
        assert!(buffer.peak() < 0.55, "peak {}", buffer.peak());
    }

    #[test]
    fn low_frequencies_are_untouched() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -30.0).unwrap();
        let mut effect = DeEsser::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..256 {
            let mut buffer = tone(300.0, 0.5, 1024);
            effect.process(&mut buffer);
        }
        let mut buffer = tone(300.0, 0.5, 1024);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.5).abs() < 0.02, "peak {}", buffer.peak());
    }

    #[test]
    fn output_is_always_finite() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -40.0).unwrap();
        params.set_f32("ratio", 10.0).unwrap();
        params.set_f32("frequency", 2_000.0).unwrap();
        let mut effect = DeEsser::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..200 {
            let mut buffer = tone(2_000.0, 1.0, 1024);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 4.0, "{sample}");
                }
            }
        }
    }
}
