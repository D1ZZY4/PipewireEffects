use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const MAX_MAKEUP_DB: f32 = 24.0;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("threshold", "Threshold", ParamUnit::Decibel)
            .range(-30.0, 0.0, 0.0)
            .step(0.1)
            .group("Maximizer")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(1.0, 100.0, 25.0)
            .logarithmic()
            .group("Maximizer")
            .build(),
    ]
}

pub struct Maximizer {
    threshold: Knob,
    release: Knob,
    gain: f32,
    reference: f32,
    rate: u32,
    release_coefficient: f32,
    attack_coefficient: f32,
    reference_coefficient: f32,
}

impl Maximizer {
    pub fn new(params: ParamSet) -> Self {
        let mut effect = Self {
            threshold: Knob::new(&params, "threshold", 0.0),
            release: Knob::new(&params, "release", 25.0),
            gain: 1.0,
            reference: 0.0,
            rate: 48_000,
            release_coefficient: 0.0,
            attack_coefficient: 0.0,
            reference_coefficient: 0.0,
        };
        effect.update_ballistics();
        effect
    }

    fn update_ballistics(&mut self) {
        self.release_coefficient = math::one_pole_alpha(self.release.value(), self.rate);
        self.attack_coefficient = math::one_pole_alpha(0.5, self.rate);
        self.reference_coefficient = math::one_pole_alpha(300.0, self.rate);
    }
}

impl Effect for Maximizer {
    fn id(&self) -> &'static str {
        "maximizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(1);
        self.update_ballistics();
        self.reset();
    }

    fn reset(&mut self) {
        self.gain = 1.0;
        self.reference = 0.0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_ballistics();
        let ceiling = math::db_to_gain(self.threshold.value()).clamp(1.0e-3, 1.0);
        let limit = math::db_to_gain(MAX_MAKEUP_DB);
        let release = self.release_coefficient;
        let attack = self.attack_coefficient;
        let reference_coefficient = self.reference_coefficient;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for frame in 0..frames {
            let mut peak = 0.0f32;
            for channel in 0..channels {
                peak = peak.max(buffer.channel(channel)[frame].abs());
            }
            self.reference += (peak - self.reference) * reference_coefficient;
            if !self.reference.is_finite() {
                self.reference = 0.0;
            }
            let makeup = (ceiling / self.reference.max(1.0e-6)).clamp(1.0, limit);
            let target = if peak > ceiling { ceiling / peak } else { 1.0 };
            let coefficient = if target < self.gain { attack } else { release };
            self.gain += (target - self.gain) * coefficient;
            if !self.gain.is_finite() {
                self.gain = 1.0;
            }
            let scale = self.gain * makeup;
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                plan[frame] = math::sanitize((plan[frame] * scale).clamp(-ceiling, ceiling));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample =
                    amplitude * (std::f32::consts::TAU * 500.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    fn settle(threshold: f32, amplitude: f32) -> f32 {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", threshold).unwrap();
        let mut effect = Maximizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut peak = 0.0f32;
        for _ in 0..64 {
            let mut block = tone(amplitude, 4_096);
            effect.process(&mut block);
            peak = peak.max(block.peak());
        }
        peak
    }

    #[test]
    fn a_loud_signal_never_passes_the_ceiling() {
        let peak = settle(-6.0, 2.0);
        assert!(peak <= math::db_to_gain(-6.0) + 1.0e-3, "{peak}");
    }

    #[test]
    fn a_quiet_signal_is_brought_up() {
        let peak = settle(0.0, 0.02);
        assert!(peak > 0.25, "the maximiser did not raise the level: {peak}");
    }

    #[test]
    fn a_signal_at_the_ceiling_is_left_alone() {
        let peak = settle(0.0, 0.5);
        assert!((peak - 1.0).abs() < 0.02, "{peak}");
    }

    #[test]
    fn the_output_stays_finite_under_extremes() {
        let peak = settle(-30.0, 4.0);
        assert!(
            peak.is_finite() && peak <= math::db_to_gain(-30.0) + 1.0e-3,
            "{peak}"
        );
    }

    #[test]
    fn silence_is_left_as_silence() {
        let peak = settle(0.0, 0.0);
        assert_eq!(peak, 0.0);
    }
}
