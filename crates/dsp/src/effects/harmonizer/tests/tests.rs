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

fn chord_run(chord: f32, note: f32, level1: f32, level2: f32) -> Vec<f32> {
    run(
        &[
            ("chord_mode", 1.0),
            ("chord", chord),
            ("note", note),
            ("level1", level1),
            ("level2", level2),
            ("mix", 100.0),
        ],
        24,
    )
}

#[test]
fn chord_mode_defaults_to_off() {
    let params = ParamSet::new(specs());
    let effect = Harmonizer::new(params);
    assert!(!effect.chord_mode.flag());
    assert_eq!(effect.note.index(), 0);
    assert_eq!(effect.chord.index(), 0);
    assert!((effect.intervals().0 - 7.0).abs() < 1.0e-6);
    assert!((effect.intervals().1 - 12.0).abs() < 1.0e-6);
}

#[test]
fn chord_mode_applies_the_major_triad_intervals() {
    let samples = chord_run(0.0, 0.0, 100.0, 0.0);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!((measured - 554.37).abs() < 20.0, "{measured}");
}

#[test]
fn chord_mode_applies_the_minor_triad_intervals() {
    let samples = chord_run(1.0, 0.0, 100.0, 0.0);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!((measured - 523.25).abs() < 20.0, "{measured}");
}

#[test]
fn chord_mode_uses_the_seventh_of_a_seventh_chord() {
    let samples = chord_run(6.0, 0.0, 0.0, 100.0);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!((measured - 783.99).abs() < 25.0, "{measured}");
}

#[test]
fn the_root_note_transposes_the_chord() {
    let base = chord_run(0.0, 0.0, 100.0, 0.0);
    let raised = chord_run(0.0, 1.0, 100.0, 0.0);
    let base_pitch = dominant(&base[base.len() - 8_192..]);
    let raised_pitch = dominant(&raised[raised.len() - 8_192..]);
    assert!((base_pitch - 554.37).abs() < 20.0, "{base_pitch}");
    assert!(
        raised_pitch > base_pitch + 20.0,
        "{raised_pitch} did not transpose above {base_pitch}"
    );
}

#[test]
fn the_root_note_offsets_the_chord_intervals() {
    let params = ParamSet::new(specs());
    params.set_f32("chord_mode", 1.0).unwrap();
    params.set_f32("chord", 0.0).unwrap();
    params.set_f32("note", 2.0).unwrap();
    let effect = Harmonizer::new(params);
    let (first, second) = effect.intervals();
    assert!((first - 6.0).abs() < 1.0e-6, "{first}");
    assert!((second - 9.0).abs() < 1.0e-6, "{second}");
}

#[test]
fn chord_mode_ignores_the_manual_intervals() {
    let params = ParamSet::new(specs());
    params.set_f32("chord_mode", 1.0).unwrap();
    params.set_f32("chord", 0.0).unwrap();
    params.set_f32("interval1", 24.0).unwrap();
    params.set_f32("interval2", -24.0).unwrap();
    let effect = Harmonizer::new(params);
    let (first, second) = effect.intervals();
    assert!((first - 4.0).abs() < 1.0e-6, "{first}");
    assert!((second - 7.0).abs() < 1.0e-6, "{second}");
}

#[test]
fn every_chord_publishes_two_rising_intervals() {
    for index in 0..CHORDS.len() {
        let params = ParamSet::new(specs());
        params.set_f32("chord_mode", 1.0).unwrap();
        params.set_f32("chord", index as f32).unwrap();
        let effect = Harmonizer::new(params);
        let (first, second) = effect.intervals();
        assert!(first > 0.0 && second > first, "{index}: {first}, {second}");
    }
}
