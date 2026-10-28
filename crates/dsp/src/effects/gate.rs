use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::dynamics::ballistics;
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("threshold", "Threshold", ParamUnit::Decibel)
            .range(-80.0, 0.0, -40.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::float("range", "Range", ParamUnit::Decibel)
            .range(0.0, 90.0, 40.0)
            .step(1.0)
            .group("Detector")
            .build(),
        ParamSpec::float("attack", "Attack", ParamUnit::Milliseconds)
            .range(0.1, 100.0, 5.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("hold", "Hold", ParamUnit::Milliseconds)
            .range(0.0, 2000.0, 50.0)
            .step(5.0)
            .group("Timing")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(5.0, 2000.0, 300.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct Gate {
    threshold: Knob,
    range: Knob,
    attack: Knob,
    hold: Knob,
    release: Knob,
    mix: Knob,
    envelope: f32,
    hold_frames: u32,
    attack_coefficient: f32,
    release_coefficient: f32,
    rate: u32,
}

impl Gate {
    pub fn new(params: ParamSet) -> Self {
        let mut effect = Self {
            threshold: Knob::new(&params, "threshold", -40.0),
            range: Knob::new(&params, "range", 40.0),
            attack: Knob::new(&params, "attack", 5.0),
            hold: Knob::new(&params, "hold", 50.0),
            release: Knob::new(&params, "release", 300.0),
            mix: Knob::new(&params, "mix", 100.0),
            envelope: 1.0,
            hold_frames: 0,
            attack_coefficient: 0.0,
            release_coefficient: 0.0,
            rate: 48_000,
        };
        effect.update_ballistics();
        effect
    }

    fn update_ballistics(&mut self) {
        let (attack, release) = ballistics(self.attack.value(), self.release.value(), self.rate);
        self.attack_coefficient = attack;
        self.release_coefficient = release;
    }
}

impl Effect for Gate {
    fn id(&self) -> &'static str {
        "gate"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.update_ballistics();
        self.reset();
    }

    fn reset(&mut self) {
        self.envelope = 1.0;
        self.hold_frames = 0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_ballistics();
        let threshold = self.threshold.value();
        let closed_gain = math::db_to_gain(-self.range.value());
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let attack = self.attack_coefficient;
        let release = self.release_coefficient;
        let hold_frames = (self.hold.value() / 1000.0 * self.rate as f32).max(0.0) as u32;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for frame in 0..frames {
            let mut peak = 0.0f32;
            for channel in 0..channels {
                peak = peak.max(buffer.channel_mut(channel)[frame].abs());
            }
            let open = math::gain_to_db(peak) >= threshold;
            if open {
                self.hold_frames = hold_frames;
            } else if self.hold_frames > 0 {
                self.hold_frames -= 1;
            }
            let target = if open || self.hold_frames > 0 {
                1.0
            } else {
                closed_gain
            };
            let coefficient = if target < self.envelope {
                release
            } else {
                attack
            };
            self.envelope += (target - self.envelope) * coefficient;
            if !self.envelope.is_finite() {
                self.envelope = 0.0;
            }
            let gain = self.envelope;
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                let dry = plan[frame];
                plan[frame] = math::sanitize(dry * (1.0 - mix) + dry * gain * mix);
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
                    amplitude * (std::f32::consts::TAU * 250.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    #[test]
    fn loud_signal_stays_open() {
        let params = ParamSet::new(specs());
        let mut effect = Gate::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.5, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.5).abs() < 0.01, "{}", buffer.peak());
    }

    #[test]
    fn quiet_signal_is_attenuated_by_the_range() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -20.0).unwrap();
        params.set_f32("range", 60.0).unwrap();
        params.set_f32("hold", 0.0).unwrap();
        params.set_f32("release", 5.0).unwrap();
        let mut effect = Gate::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.001, 4096);
        for _ in 0..256 {
            buffer = tone(0.001, 4096);
            effect.process(&mut buffer);
        }
        let expected = 0.001 * math::db_to_gain(-60.0);
        assert!(
            (buffer.peak() - expected).abs() < 1.0e-5,
            "{} {}",
            buffer.peak(),
            expected
        );
    }

    #[test]
    fn hold_delays_the_close() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -20.0).unwrap();
        params.set_f32("hold", 500.0).unwrap();
        let mut effect = Gate::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut loud = tone(0.5, 1024);
        effect.process(&mut loud);
        let mut quiet = tone(0.0001, 512);
        effect.process(&mut quiet);
        assert!(
            quiet.peak() > 0.00005,
            "gate closed too early: {}",
            quiet.peak()
        );
    }
}
