use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;
use crate::r128::Loudness;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("target", "Target loudness", ParamUnit::Lufs)
            .range(-36.0, -5.0, -23.0)
            .step(0.5)
            .group("Level")
            .build(),
        ParamSpec::float("max_gain", "Maximum gain", ParamUnit::Decibel)
            .range(0.0, 40.0, 20.0)
            .step(0.5)
            .group("Level")
            .build(),
        ParamSpec::float("silence_threshold", "Silence threshold", ParamUnit::Lufs)
            .range(-80.0, -40.0, -70.0)
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
    loudness: Loudness,
    gain_db: f32,
    rate: u32,
}

impl AutoGain {
    pub fn new(params: ParamSet) -> Self {
        Self {
            target: Knob::new(&params, "target", -23.0),
            max_gain: Knob::new(&params, "max_gain", 20.0),
            silence_threshold: Knob::new(&params, "silence_threshold", -70.0),
            speed: Knob::new(&params, "speed", 1000.0),
            loudness: Loudness::new(48_000),
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
        self.loudness = Loudness::new(self.rate);
        self.reset();
    }

    fn reset(&mut self) {
        self.gain_db = 0.0;
        self.loudness.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let frames = buffer.frames();
        if frames == 0 {
            return;
        }
        self.loudness.process(buffer);

        let start_db = self.gain_db;
        if let Some(measured) = self.loudness.integrated()
            && measured > self.silence_threshold.value()
        {
            let wanted = (self.target.value() - measured).clamp(0.0, self.max_gain.value());
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
                    amplitude * (std::f32::consts::TAU * 997.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    fn feed(effect: &mut AutoGain, amplitude: f32, blocks: usize) {
        for _ in 0..blocks {
            let mut buffer = tone(amplitude, 4800);
            effect.process(&mut buffer);
        }
    }

    fn measured(amplitude: f32, blocks: usize) -> f32 {
        let mut loudness = Loudness::new(48_000);
        for _ in 0..blocks {
            let buffer = tone(amplitude, 4800);
            loudness.process(&buffer);
        }
        loudness.integrated().expect("a level was measured")
    }

    #[test]
    fn a_full_scale_sine_measures_about_zero_lufs() {
        let value = measured(1.0, 40);
        assert!(value.abs() < 0.6, "{value}");
    }

    #[test]
    fn the_standard_calibration_holds() {
        let amplitude = 10f32.powf(-23.0 / 20.0);
        let value = measured(amplitude, 40);
        assert!((value + 23.0).abs() < 0.4, "{value}");
    }

    #[test]
    fn a_ten_decibel_step_moves_the_measurement_by_ten() {
        let loud = measured(0.5, 40);
        let quiet = measured(0.5 / 10f32.powf(10.0 / 20.0), 40);
        assert!((loud - quiet - 10.0).abs() < 0.3, "{loud} {quiet}");
    }

    #[test]
    fn silence_is_not_measured() {
        let mut loudness = Loudness::new(48_000);
        let mut silent = AudioBuffer::new(2, 4800);
        silent.configure(2, 4800).unwrap();
        for _ in 0..40 {
            loudness.process(&silent);
        }
        assert!(loudness.integrated().is_none());
    }

    #[test]
    fn quiet_material_is_lifted_towards_the_target() {
        let params = ParamSet::new(specs());
        params.set_f32("speed", 100.0).unwrap();
        let mut effect = AutoGain::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        feed(&mut effect, 0.01, 400);
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
        feed(&mut effect, 0.0001, 400);
        assert!(effect.gain_db <= 6.001, "{}", effect.gain_db);
    }

    #[test]
    fn silence_does_not_change_the_gain() {
        let params = ParamSet::new(specs());
        let mut effect = AutoGain::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut silent = AudioBuffer::new(2, 4800);
        silent.configure(2, 4800).unwrap();
        for _ in 0..100 {
            effect.process(&mut silent);
        }
        assert_eq!(effect.gain_db, 0.0);
    }

    #[test]
    fn no_gain_is_applied_when_the_loudness_already_matches() {
        let params = ParamSet::new(specs());
        params.set_f32("target", -23.0).unwrap();
        let mut effect = AutoGain::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        feed(&mut effect, 10f32.powf(-23.0 / 20.0), 60);
        assert!(effect.gain_db.abs() < 0.5, "{}", effect.gain_db);
    }
}
