use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::{self, MAX_CHANNELS};
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("target", "Target level", ParamUnit::Decibel)
            .range(-30.0, 0.0, -14.0)
            .step(0.5)
            .group("Level")
            .build(),
        ParamSpec::float("max_gain", "Maximum gain", ParamUnit::Decibel)
            .range(0.0, 40.0, 20.0)
            .step(0.5)
            .group("Level")
            .build(),
        ParamSpec::float("silence_threshold", "Silence threshold", ParamUnit::Decibel)
            .range(-80.0, -30.0, -50.0)
            .step(1.0)
            .group("Level")
            .build(),
        ParamSpec::float("speed", "Speed", ParamUnit::Milliseconds)
            .range(100.0, 5000.0, 1000.0)
            .logarithmic()
            .group("Timing")
            .build(),
    ]
}

pub struct AutoGain {
    target: Knob,
    max_gain: Knob,
    silence_threshold: Knob,
    speed: Knob,
    gain_db: f32,
    rate: u32,
}

impl AutoGain {
    pub fn new(params: ParamSet) -> Self {
        Self {
            target: Knob::new(&params, "target", -14.0),
            max_gain: Knob::new(&params, "max_gain", 20.0),
            silence_threshold: Knob::new(&params, "silence_threshold", -50.0),
            speed: Knob::new(&params, "speed", 1000.0),
            gain_db: 0.0,
            rate: 48_000,
        }
    }
}

impl Effect for AutoGain {
    fn id(&self) -> &'static str {
        "autogain"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.reset();
    }

    fn reset(&mut self) {
        self.gain_db = 0.0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let frames = buffer.frames();
        if frames == 0 {
            return;
        }
        let mean_square = util::mean_square(buffer);
        let level_db = if mean_square <= 0.0 {
            math::MIN_GAIN
        } else {
            10.0 * mean_square.log10()
        };

        let start_db = self.gain_db;
        if level_db > self.silence_threshold.value() {
            let wanted = (self.target.value() - level_db).clamp(0.0, self.max_gain.value());
            let block_ms = frames as f32 / self.rate.max(1) as f32 * 1000.0;
            let alpha = 1.0 - (-block_ms / self.speed.value().max(1.0)).exp();
            self.gain_db += (wanted - self.gain_db) * alpha;
        }
        if !self.gain_db.is_finite() {
            self.gain_db = 0.0;
        }

        let start = math::db_to_gain(start_db);
        let end = math::db_to_gain(self.gain_db);
        let step = (end - start) / frames as f32;
        let channels = buffer.channels().min(MAX_CHANNELS);
        for channel in 0..channels {
            let mut gain = start;
            for sample in buffer.channel_mut(channel) {
                *sample = math::sanitize(*sample * gain);
                gain += step;
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
                    amplitude * (std::f32::consts::TAU * 440.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    #[test]
    fn quiet_material_is_lifted_towards_the_target() {
        let params = ParamSet::new(specs());
        params.set_f32("speed", 100.0).unwrap();
        let mut effect = AutoGain::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..200 {
            let mut buffer = tone(0.01, 1024);
            effect.process(&mut buffer);
            let _ = buffer;
        }
        assert!(effect.gain_db > 5.0, "{}", effect.gain_db);
        assert!(effect.gain_db <= 20.0);
    }

    #[test]
    fn the_gain_is_capped_by_the_maximum() {
        let params = ParamSet::new(specs());
        params.set_f32("speed", 100.0).unwrap();
        params.set_f32("max_gain", 6.0).unwrap();
        let mut effect = AutoGain::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..500 {
            let mut buffer = tone(0.0001, 1024);
            effect.process(&mut buffer);
        }
        assert!(effect.gain_db <= 6.001, "{}", effect.gain_db);
    }

    #[test]
    fn silence_does_not_change_the_gain() {
        let params = ParamSet::new(specs());
        let mut effect = AutoGain::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut silent = AudioBuffer::new(2, 1024);
        silent.configure(2, 1024).unwrap();
        for _ in 0..100 {
            effect.process(&mut silent);
        }
        assert_eq!(effect.gain_db, 0.0);
    }

    #[test]
    fn no_gain_is_applied_when_the_level_already_matches() {
        let params = ParamSet::new(specs());
        params.set_f32("target", -6.0).unwrap();
        let mut effect = AutoGain::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let amplitude = 10f32.powf(-6.0 / 20.0) * std::f32::consts::SQRT_2;
        for _ in 0..50 {
            let mut buffer = tone(amplitude, 1024);
            effect.process(&mut buffer);
        }
        assert!(effect.gain_db.abs() < 0.5, "{}", effect.gain_db);
    }
}
