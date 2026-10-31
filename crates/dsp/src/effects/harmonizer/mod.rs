use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::vocoder::Vocoder;
use crate::knob::Knob;

const CHANNELS: usize = 2;
const MAX_FRAMES: usize = 16_384;

const NOTE_LABELS: [&str; 12] = [
    mark("C"),
    mark("C sharp"),
    mark("D"),
    mark("D sharp"),
    mark("E"),
    mark("F"),
    mark("F sharp"),
    mark("G"),
    mark("G sharp"),
    mark("A"),
    mark("A sharp"),
    mark("B"),
];

const CHORD_LABELS: [&str; 8] = [
    mark("Major triad"),
    mark("Minor triad"),
    mark("Diminished triad"),
    mark("Augmented triad"),
    mark("Sus2 triad"),
    mark("Sus4 triad"),
    mark("Dominant seventh"),
    mark("Major seventh"),
];

const CHORDS: [(i32, i32); 8] = [
    (4, 7),
    (3, 7),
    (3, 6),
    (4, 8),
    (2, 7),
    (5, 7),
    (4, 10),
    (4, 11),
];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::boolean("chord_mode", "Chord mode", false).group("Chord"),
        ParamSpec::choice("note", "Root note", &NOTE_LABELS, 0).group("Chord"),
        ParamSpec::choice("chord", "Chord", &CHORD_LABELS, 0).group("Chord"),
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
    chord_mode: Knob,
    note: Knob,
    chord: Knob,
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
            chord_mode: Knob::new(&params, "chord_mode", 0.0),
            note: Knob::new(&params, "note", 0.0),
            chord: Knob::new(&params, "chord", 0.0),
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

    fn intervals(&self) -> (f32, f32) {
        if !self.chord_mode.flag() {
            return (self.interval1.value(), self.interval2.value());
        }
        let index = self.chord.index().min(CHORDS.len() - 1);
        let root = (self.note.index() % 12) as i32;
        let (first, second) = CHORDS[index];
        ((first + root) as f32, (second + root) as f32)
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
        let (interval1, interval2) = self.intervals();
        let ratio1 = Self::ratio(interval1);
        let ratio2 = Self::ratio(interval2);

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
