use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::dynamics::{ballistics, filtered_measure};
use crate::effects::sidechain::{MODES as SIDECHAIN_MODES, SidechainFilter};
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
        ParamSpec::choice("hpf_mode", "High-pass", &SIDECHAIN_MODES, 0).group("Sidechain"),
        ParamSpec::float("hpf_frequency", "High-pass frequency", ParamUnit::Hertz)
            .range(10.0, 2_000.0, 10.0)
            .logarithmic()
            .group("Sidechain")
            .build(),
        ParamSpec::choice("lpf_mode", "Low-pass", &SIDECHAIN_MODES, 0).group("Sidechain"),
        ParamSpec::float("lpf_frequency", "Low-pass frequency", ParamUnit::Hertz)
            .range(1_000.0, 20_000.0, 20_000.0)
            .logarithmic()
            .group("Sidechain")
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
    high_mode: Knob,
    high_frequency: Knob,
    low_mode: Knob,
    low_frequency: Knob,
    sidechain: SidechainFilter,
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
            high_mode: Knob::new(&params, "hpf_mode", 0.0),
            high_frequency: Knob::new(&params, "hpf_frequency", 10.0),
            low_mode: Knob::new(&params, "lpf_mode", 0.0),
            low_frequency: Knob::new(&params, "lpf_frequency", 20_000.0),
            sidechain: SidechainFilter::new(48_000),
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

    fn sidechain_update(&mut self) {
        self.sidechain.update(
            self.high_mode.index(),
            self.high_frequency.value(),
            self.low_mode.index(),
            self.low_frequency.value(),
        );
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
        self.sidechain.prepare(self.rate);
        self.update_ballistics();
        self.reset();
    }

    fn reset(&mut self) {
        self.sidechain.reset();
        self.envelope = 1.0;
        self.open = true;
        self.hold_frames = 0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_ballistics();
        self.sidechain_update();
        let closed_gain = math::db_to_gain(-self.range.value());
        let curve = self.curve.value().max(0.1);
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let attack = self.attack_coefficient;
        let release = self.release_coefficient;
        let hold_frames = (self.hold.value() / 1000.0 * self.rate as f32).max(0.0) as u32;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for frame in 0..frames {
            let (peak, _) = filtered_measure(&mut self.sidechain, buffer, channels, frame);
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
#[path = "tests/tests.rs"]
mod tests;
