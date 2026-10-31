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
#[path = "tests/tests.rs"]
mod tests;
