#![allow(unused_imports)]

pub use std::f32::consts::PI;

pub use pipewire_effects_core::{AudioBlock, AudioEffect, AudioFormat, ParamValue};
pub use pipewire_effects_dsp::effects::{
    eq::ParametricEqEffect, gain::GainEffect, limiter::LimiterEffect,
};
pub use pipewire_effects_dsp::util::{db_to_linear, linear_to_db};

pub const RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
pub const BLOCK_FRAMES: usize = 480;

pub fn format() -> AudioFormat {
    AudioFormat::new(RATE, CHANNELS as u16)
}

pub fn stereo_block(samples: &mut [f32]) -> AudioBlock<'_> {
    AudioBlock::new(samples, format())
}

pub fn sine(frequency: f32, frames: usize, amplitude: f32) -> Vec<f32> {
    let mut samples = vec![0.0_f32; frames * CHANNELS];
    for frame in 0..frames {
        #[allow(clippy::cast_precision_loss)]
        let phase = 2.0 * PI * frequency * frame as f32 / RATE as f32;
        let value = phase.sin() * amplitude;
        samples[frame * CHANNELS] = value;
        samples[frame * CHANNELS + 1] = value;
    }
    samples
}

pub fn process_in_blocks(effect: &mut dyn AudioEffect, samples: &mut [f32]) {
    let mut offset = 0;
    while offset < samples.len() {
        let end = (offset + BLOCK_FRAMES * CHANNELS).min(samples.len());
        let mut block = stereo_block(&mut samples[offset..end]);
        effect.process(&mut block);
        offset = end;
    }
}

pub fn tone_magnitude(samples: &[f32], frequency: f32) -> f32 {
    let mono: Vec<f32> = samples
        .chunks_exact(CHANNELS)
        .map(|frame| frame[0])
        .collect();
    let n = mono.len();
    if n == 0 {
        return 0.0;
    }

    #[allow(clippy::cast_precision_loss)]
    let n_f = n as f32;
    let k = (0.5 + n_f * frequency / RATE as f32).floor();
    let omega = 2.0 * PI * k / n_f;
    let coefficient = 2.0 * omega.cos();

    let (mut s1, mut s2) = (0.0_f32, 0.0_f32);
    for sample in &mono {
        let s0 = sample + coefficient * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = (s1 * s1 + s2 * s2 - coefficient * s1 * s2).max(0.0);
    power.sqrt() * 2.0 / n_f
}

pub fn settled_tail(samples: &[f32]) -> &[f32] {
    &samples[samples.len() / 2..]
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()))
}

#[allow(dead_code)]
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let count = samples.len() as f32;
    (samples.iter().map(|s| s * s).sum::<f32>() / count).sqrt()
}
