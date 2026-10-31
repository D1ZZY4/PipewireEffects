use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::vocoder::Vocoder;
use crate::knob::Knob;

const CHANNELS: usize = 2;
const MAX_FRAMES: usize = 16_384;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("interval1", "Voice 1 interval", ParamUnit::Semitones)
            .range(-24.0, 24.0, 7.0)
            .step(1.0)
            .group("Voice 1")
            .build(),
        ParamSpec::float("level1", "Voice 1 level", ParamUnit::Percent)
            .range(0.0, 100.0, 60.0)
            .step(1.0)
            .group("Voice 1")
            .build(),
        ParamSpec::float("interval2", "Voice 2 interval", ParamUnit::Semitones)
            .range(-24.0, 24.0, 12.0)
            .step(1.0)
            .group("Voice 2")
            .build(),
        ParamSpec::float("level2", "Voice 2 level", ParamUnit::Percent)
            .range(0.0, 100.0, 40.0)
            .step(1.0)
            .group("Voice 2")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 60.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct Harmonizer {
    voice1: Vocoder,
    voice2: Vocoder,
    interval1: Knob,
    level1: Knob,
    interval2: Knob,
    level2: Knob,
    mix: Knob,
    dry: Vec<Vec<f32>>,
    first: Vec<Vec<f32>>,
}

impl Harmonizer {
    pub fn new(params: ParamSet) -> Self {
        Self {
            voice1: Vocoder::new(),
            voice2: Vocoder::new(),
            interval1: Knob::new(&params, "interval1", 7.0),
            level1: Knob::new(&params, "level1", 60.0),
            interval2: Knob::new(&params, "interval2", 12.0),
            level2: Knob::new(&params, "level2", 40.0),
            mix: Knob::new(&params, "mix", 60.0),
            dry: vec![vec![0.0; MAX_FRAMES]; CHANNELS],
            first: vec![vec![0.0; MAX_FRAMES]; CHANNELS],
        }
    }

    fn ratio(semitones: f32) -> f32 {
        (semitones.clamp(-24.0, 24.0) / 12.0).exp2()
    }
}

impl Effect for Harmonizer {
    fn id(&self) -> &'static str {
        "harmonizer"
    }

    fn prepare(&mut self, _format: AudioFormat) {
        self.reset();
    }

    fn reset(&mut self) {
        self.voice1.reset();
        self.voice2.reset();
    }

    fn latency_frames(&self) -> usize {
        crate::effects::vocoder::LATENCY_FRAMES
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let frames = buffer.frames().min(MAX_FRAMES);
        let channels = buffer.channels().min(CHANNELS);
        if frames == 0 || channels == 0 {
            return;
        }
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let level1 = (self.level1.value() / 100.0).clamp(0.0, 1.0);
        let level2 = (self.level2.value() / 100.0).clamp(0.0, 1.0);
        let ratio1 = Self::ratio(self.interval1.value());
        let ratio2 = Self::ratio(self.interval2.value());

        for channel in 0..channels {
            self.dry[channel][..frames].copy_from_slice(&buffer.channel(channel)[..frames]);
        }
        self.voice1.process(buffer, ratio1, 1.0);
        for channel in 0..channels {
            self.first[channel][..frames].copy_from_slice(&buffer.channel(channel)[..frames]);
            buffer
                .channel_mut(channel)
                .copy_from_slice(&self.dry[channel][..frames]);
        }
        self.voice2.process(buffer, ratio2, 1.0);
        for channel in 0..channels {
            let plan = buffer.channel_mut(channel);
            for ((slot, dry), first) in plan
                .iter_mut()
                .zip(self.dry[channel].iter())
                .zip(self.first[channel].iter())
                .take(frames)
            {
                let voices = first * level1 + *slot * level2;
                *slot = math::sanitize(dry * (1.0 - mix) + voices * mix);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
