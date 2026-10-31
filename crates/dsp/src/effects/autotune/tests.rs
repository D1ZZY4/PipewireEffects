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

fn run(frequency: f32, settings: &[(&str, f32)], blocks: usize) -> Vec<f32> {
    let params = ParamSet::new(specs());
    for (id, value) in settings {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = Autotune::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    let mut collected = Vec::new();
    for block in 0..blocks {
        let mut buffer = tone(frequency, BLOCK, block * BLOCK);
        effect.process(&mut buffer);
        collected.extend_from_slice(buffer.channel(0));
    }
    collected
}

#[test]
fn a_detuned_pitch_is_pulled_to_the_nearest_note() {
    let samples = run(226.0, &[], 24);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!(
        (measured - 220.0).abs() < 8.0,
        "the pitch was not corrected to A3: {measured}"
    );
}

#[test]
fn zero_strength_leaves_the_pitch_alone() {
    let samples = run(226.0, &[("strength", 0.0)], 24);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!(
        (measured - 226.0).abs() < 8.0,
        "the pitch moved with zero strength: {measured}"
    );
}

#[test]
fn zero_mix_is_transparent() {
    let mut effect = {
        let params = ParamSet::new(specs());
        params.set_f32("mix", 0.0).unwrap();
        let mut effect = Autotune::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        effect
    };
    let mut buffer = tone(226.0, BLOCK, 0);
    let reference = buffer.channel(0).to_vec();
    effect.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
        assert!((processed - original).abs() < 1.0e-6);
    }
}

#[test]
fn the_output_stays_finite_under_the_widest_settings() {
    let mut effect = {
        let params = ParamSet::new(specs());
        params.set_f32("strength", 100.0).unwrap();
        params.set_f32("speed", 5.0).unwrap();
        params.set_f32("min_frequency", 50.0).unwrap();
        params.set_f32("max_frequency", 2_000.0).unwrap();
        let mut effect = Autotune::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        effect
    };
    for block in 0..16 {
        let mut buffer = tone(58.0, BLOCK, block * BLOCK);
        buffer.channel_mut(0)[0] = 4.0;
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 32.0, "{sample}");
            }
        }
    }
}
