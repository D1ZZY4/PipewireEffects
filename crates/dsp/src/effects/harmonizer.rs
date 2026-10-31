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
mod tests {
    use std::f32::consts::TAU;

    use crate::fft::{Fft, hann};

    use super::*;

    const RATE: u32 = 48_000;
    const BLOCK: usize = 1_024;

    fn tone(frequency: f32, frames: usize, offset: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                let phase = TAU * frequency * (offset + index) as f32 / RATE as f32;
                *sample = 0.5 * phase.sin();
            }
        }
        buffer
    }

    fn dominant(samples: &[f32]) -> f32 {
        let size = 8_192;
        let fft = Fft::new(size);
        let window = hann(size);
        let mut re = vec![0.0f32; size];
        let mut im = vec![0.0f32; size];
        for (index, slot) in re.iter_mut().enumerate() {
            *slot = samples.get(index).copied().unwrap_or(0.0) * window[index];
        }
        fft.forward(&mut re, &mut im);
        let mut best = 1usize;
        for bin in 1..size / 2 {
            let power = re[bin] * re[bin] + im[bin] * im[bin];
            if power > re[best] * re[best] + im[best] * im[best] {
                best = bin;
            }
        }
        best as f32 * RATE as f32 / size as f32
    }

    fn run(settings: &[(&str, f32)], blocks: usize) -> Vec<f32> {
        let params = ParamSet::new(specs());
        for (id, value) in settings {
            params.set_f32(id, *value).unwrap();
        }
        let mut effect = Harmonizer::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        let mut collected = Vec::new();
        for block in 0..blocks {
            let mut buffer = tone(440.0, BLOCK, block * BLOCK);
            effect.process(&mut buffer);
            collected.extend_from_slice(buffer.channel(0));
        }
        collected
    }

    #[test]
    fn a_voice_an_octave_up_is_generated() {
        let samples = run(
            &[
                ("interval1", 12.0),
                ("level1", 100.0),
                ("level2", 0.0),
                ("mix", 100.0),
            ],
            24,
        );
        let measured = dominant(&samples[samples.len() - 8_192..]);
        assert!((measured - 880.0).abs() < 20.0, "{measured}");
    }

    #[test]
    fn a_fifth_voice_is_generated() {
        let samples = run(
            &[
                ("interval1", 7.0),
                ("level1", 100.0),
                ("level2", 0.0),
                ("mix", 100.0),
            ],
            24,
        );
        let measured = dominant(&samples[samples.len() - 8_192..]);
        assert!((measured - 659.3).abs() < 20.0, "{measured}");
    }

    #[test]
    fn zero_mix_is_transparent() {
        let mut effect = {
            let params = ParamSet::new(specs());
            params.set_f32("mix", 0.0).unwrap();
            let mut effect = Harmonizer::new(params);
            effect.prepare(AudioFormat::new(RATE, 2));
            effect
        };
        let mut buffer = tone(440.0, BLOCK, 0);
        let reference = buffer.channel(0).to_vec();
        effect.process(&mut buffer);
        for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
            assert!((processed - original).abs() < 1.0e-6);
        }
    }

    #[test]
    fn the_output_stays_finite_at_the_extremes() {
        let mut effect = {
            let params = ParamSet::new(specs());
            params.set_f32("interval1", -24.0).unwrap();
            params.set_f32("interval2", 24.0).unwrap();
            params.set_f32("level1", 100.0).unwrap();
            params.set_f32("level2", 100.0).unwrap();
            params.set_f32("mix", 100.0).unwrap();
            let mut effect = Harmonizer::new(params);
            effect.prepare(AudioFormat::new(RATE, 2));
            effect
        };
        for block in 0..16 {
            let mut buffer = tone(120.0, BLOCK, block * BLOCK);
            buffer.channel_mut(0)[0] = 4.0;
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 32.0, "{sample}");
                }
            }
        }
    }
}
