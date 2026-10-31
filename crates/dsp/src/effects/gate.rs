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
        ParamSpec::float("curve", "Curve", ParamUnit::DecibelWidth)
            .range(0.0, 60.0, 6.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::float("hysteresis", "Hysteresis", ParamUnit::DecibelWidth)
            .range(0.0, 30.0, 0.0)
            .step(0.5)
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
    curve: Knob,
    hysteresis: Knob,
    attack: Knob,
    hold: Knob,
    release: Knob,
    mix: Knob,
    envelope: f32,
    open: bool,
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
            curve: Knob::new(&params, "curve", 6.0),
            hysteresis: Knob::new(&params, "hysteresis", 0.0),
            attack: Knob::new(&params, "attack", 5.0),
            hold: Knob::new(&params, "hold", 50.0),
            release: Knob::new(&params, "release", 300.0),
            mix: Knob::new(&params, "mix", 100.0),
            envelope: 1.0,
            open: true,
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

    fn target_gain(&mut self, level_db: f32, hold: u32, closed_gain: f32, curve: f32) -> f32 {
        if self.open {
            if level_db < self.threshold.value() - self.hysteresis.value() {
                self.open = false;
            }
        } else if level_db >= self.threshold.value() {
            self.open = true;
        }
        if self.open {
            self.hold_frames = hold;
        } else if self.hold_frames > 0 {
            self.hold_frames -= 1;
        }
        if self.hold_frames > 0 {
            self.open = true;
        }
        let reference = if self.open {
            self.threshold.value()
        } else {
            self.threshold.value() - self.hysteresis.value()
        };
        let depth = ((reference - level_db) / curve.max(0.1)).clamp(0.0, 1.0);
        closed_gain + (1.0 - closed_gain) * (1.0 - depth)
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
        self.open = true;
        self.hold_frames = 0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_ballistics();
        let closed_gain = math::db_to_gain(-self.range.value());
        let curve = self.curve.value().max(0.1);
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
            let level_db = math::gain_to_db(peak);
            let target = self.target_gain(level_db, hold_frames, closed_gain, curve);
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
mod tests;
