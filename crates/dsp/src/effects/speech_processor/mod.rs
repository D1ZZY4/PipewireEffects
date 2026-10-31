use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::dynamics::ballistics;
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;
use crate::r128::Loudness;

mod dereverb;

use dereverb::Dereverb;

const FLOOR_MIN_DB: f32 = -90.0;
const FLOOR_FALL: f32 = 0.25;
const FLOOR_RISE: f32 = 0.0004;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::boolean("vad", "Voice activity detection", true).group("Voice activity"),
        ParamSpec::float(
            "vad_threshold",
            "Voice threshold above the floor",
            ParamUnit::Decibel,
        )
        .range(3.0, 30.0, 9.0)
        .step(0.5)
        .group("Voice activity")
        .build(),
        ParamSpec::float("release", "Voice release", ParamUnit::Milliseconds)
            .range(50.0, 2_000.0, 300.0)
            .logarithmic()
            .group("Voice activity")
            .build(),
        ParamSpec::boolean("agc", "Automatic gain", true).group("Levelling"),
        ParamSpec::float("target", "Target loudness", ParamUnit::Lufs)
            .range(-36.0, -5.0, -20.0)
            .step(0.5)
            .group("Levelling")
            .build(),
        ParamSpec::float("max_gain", "Maximum gain", ParamUnit::Decibel)
            .range(0.0, 30.0, 18.0)
            .step(0.5)
            .group("Levelling")
            .build(),
        ParamSpec::boolean("denoise", "Noise reduction", true).group("Noise"),
        ParamSpec::float("depth", "Reduction", ParamUnit::Decibel)
            .range(0.0, 60.0, 18.0)
            .step(1.0)
            .group("Noise")
            .build(),
        ParamSpec::boolean("dereverb", "Dereverb", false).group("Reverberation"),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct SpeechProcessor {
    vad: Knob,
    vad_threshold: Knob,
    release: Knob,
    agc: Knob,
    target: Knob,
    max_gain: Knob,
    denoise: Knob,
    depth: Knob,
    dereverb: Knob,
    mix: Knob,
    measurement: Loudness,
    stage: Dereverb,
    floor_db: f32,
    voice: f32,
    reduction_db: f32,
    gain_db: f32,
    attack_coefficient: f32,
    release_coefficient: f32,
    rate: u32,
}

impl SpeechProcessor {
    pub fn new(params: ParamSet) -> Self {
        let mut effect = Self {
            vad: Knob::new(&params, "vad", 1.0),
            vad_threshold: Knob::new(&params, "vad_threshold", 9.0),
            release: Knob::new(&params, "release", 300.0),
            agc: Knob::new(&params, "agc", 1.0),
            target: Knob::new(&params, "target", -20.0),
            max_gain: Knob::new(&params, "max_gain", 18.0),
            denoise: Knob::new(&params, "denoise", 1.0),
            depth: Knob::new(&params, "depth", 18.0),
            dereverb: Knob::new(&params, "dereverb", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            measurement: Loudness::new(48_000),
            stage: Dereverb::new(),
            floor_db: FLOOR_MIN_DB,
            voice: 0.0,
            reduction_db: 0.0,
            gain_db: 0.0,
            attack_coefficient: 0.0,
            release_coefficient: 0.0,
            rate: 48_000,
        };
        effect.update_ballistics();
        effect
    }

    fn update_ballistics(&mut self) {
        let (attack, release) = ballistics(5.0, self.release.value(), self.rate);
        self.attack_coefficient = attack;
        self.release_coefficient = release;
    }

    fn track_floor(&mut self, level_db: f32) {
        let coefficient = if level_db < self.floor_db {
            FLOOR_FALL
        } else {
            FLOOR_RISE
        };
        self.floor_db += (level_db - self.floor_db) * coefficient;
        self.floor_db = self.floor_db.clamp(FLOOR_MIN_DB, 0.0);
    }

    fn track_voice(&mut self, level_db: f32) {
        if !self.vad.flag() {
            self.voice = 1.0;
            return;
        }
        let open = level_db > self.floor_db + self.vad_threshold.value();
        let coefficient = if open {
            self.attack_coefficient
        } else {
            self.release_coefficient
        };
        let wanted = if open { 1.0 } else { 0.0 };
        self.voice += (wanted - self.voice) * coefficient;
    }

    fn track_reduction(&mut self, frames: usize) {
        let wanted = if self.denoise.flag() {
            self.depth.value().clamp(0.0, 60.0) * (1.0 - self.voice)
        } else {
            0.0
        };
        let block_ms = frames as f32 / self.rate.max(1) as f32 * 1000.0;
        let alpha = 1.0 - (-block_ms / 120.0).exp();
        self.reduction_db += (wanted - self.reduction_db) * alpha;
    }

    fn track_gain(&mut self, frames: usize) {
        if !self.agc.flag() || self.voice < 0.5 {
            return;
        }
        let Some(measured) = self.measurement.short_term() else {
            return;
        };
        if measured <= FLOOR_MIN_DB {
            return;
        }
        let wanted = (self.target.value() - measured).clamp(0.0, self.max_gain.value());
        let block_ms = frames as f32 / self.rate.max(1) as f32 * 1000.0;
        let alpha = 1.0 - (-block_ms / 1_000.0).exp();
        self.gain_db += (wanted - self.gain_db) * alpha;
        if !self.gain_db.is_finite() {
            self.gain_db = 0.0;
        }
    }
}

impl Effect for SpeechProcessor {
    fn id(&self) -> &'static str {
        "speech_processor"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.measurement = Loudness::new(self.rate);
        self.update_ballistics();
        self.reset();
    }

    fn reset(&mut self) {
        self.floor_db = FLOOR_MIN_DB;
        self.voice = 0.0;
        self.reduction_db = 0.0;
        self.gain_db = 0.0;
        self.measurement.reset();
        self.stage.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let frames = buffer.frames();
        if frames == 0 {
            return;
        }
        self.update_ballistics();
        let level_db = math::gain_to_db(buffer.peak());
        self.track_floor(level_db);
        self.track_voice(level_db);
        self.track_reduction(frames);
        self.measurement.process(buffer);
        self.track_gain(frames);

        let gain = math::db_to_gain(self.gain_db - self.reduction_db);
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        if mix > 0.0 && self.dereverb.flag() {
            self.stage.process(buffer);
        }
        let channels = buffer.channels().min(MAX_CHANNELS);
        for channel in 0..channels {
            for sample in buffer.channel_mut(channel) {
                let dry = *sample;
                *sample = math::sanitize(dry * (1.0 - mix) + dry * gain * mix);
            }
        }
    }

    fn latency_frames(&self) -> usize {
        if self.dereverb.flag() {
            dereverb::LATENCY_FRAMES
        } else {
            0
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
