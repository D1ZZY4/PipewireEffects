use std::time::Instant;

use pwe_core::{AudioBuffer, AudioFormat, Effect, EffectChain, ParamSet};
use pwe_dsp::{EFFECTS, EffectDescriptor};

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const FRAMES: usize = 1024;
const BLOCKS: usize = 2_000;
const WARM_UP: usize = 64;

fn sound(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(CHANNELS, frames);
    buffer
        .configure(CHANNELS, frames)
        .expect("the buffer should configure");
    buffer
}

fn fill(buffer: &mut AudioBuffer, phase: &mut f32) {
    let step = std::f32::consts::TAU * 997.0 / RATE as f32;
    for frame in 0..buffer.frames() {
        *phase += step;
        if *phase >= std::f32::consts::TAU {
            *phase -= std::f32::consts::TAU;
        }
        let value = 0.25 * phase.sin();
        for channel in 0..CHANNELS {
            buffer.channel_mut(channel)[frame] = value;
        }
    }
}

fn build(descriptor: &EffectDescriptor) -> Box<dyn Effect> {
    (descriptor.build)(ParamSet::new(descriptor.specs()))
}

fn chain(descriptors: &[&EffectDescriptor], frames: usize) -> EffectChain {
    let mut chain = EffectChain::new(descriptors.len(), AudioFormat::new(RATE, CHANNELS));
    for descriptor in descriptors {
        chain
            .push(build(descriptor))
            .expect("the chain should accept every effect");
    }
    let mut buffer = sound(frames);
    let mut phase = 0.0;
    for _ in 0..WARM_UP {
        fill(&mut buffer, &mut phase);
        chain.process(&mut buffer);
    }
    chain
}

const ROUNDS: usize = 3;

fn best_of<F: FnMut()>(mut body: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..ROUNDS {
        let start = Instant::now();
        body();
        best = best.min(start.elapsed().as_secs_f64());
    }
    best
}

fn seconds_per_block(mut chain: EffectChain, frames: usize) -> f64 {
    let mut buffer = sound(frames);
    let mut phase = 0.0;
    best_of(|| {
        for _ in 0..BLOCKS {
            fill(&mut buffer, &mut phase);
            chain.process(&mut buffer);
            std::hint::black_box(buffer.peak());
        }
    }) / BLOCKS as f64
}

fn analyser_cost(frames: usize) -> f64 {
    let mut analyser = pwe_dsp::SpectrumAnalyzer::new();
    analyser.prepare(RATE);
    let mut buffer = sound(frames);
    let mut phase = 0.0;
    let with = best_of(|| {
        for _ in 0..BLOCKS {
            fill(&mut buffer, &mut phase);
            std::hint::black_box(analyser.push(&buffer));
        }
    });
    let without = best_of(|| {
        for _ in 0..BLOCKS {
            fill(&mut buffer, &mut phase);
            std::hint::black_box(buffer.peak());
        }
    });
    (with - without) / BLOCKS as f64
}

fn fill_cost(frames: usize) -> f64 {
    let mut buffer = sound(frames);
    let mut phase = 0.0;
    best_of(|| {
        for _ in 0..BLOCKS {
            fill(&mut buffer, &mut phase);
            std::hint::black_box(buffer.peak());
        }
    }) / BLOCKS as f64
}

fn loudness_cost(frames: usize) -> f64 {
    let mut loudness = pwe_dsp::Loudness::new(RATE);
    let mut buffer = sound(frames);
    let mut phase = 0.0;
    let with = best_of(|| {
        for _ in 0..BLOCKS {
            fill(&mut buffer, &mut phase);
            loudness.process(&buffer);
            std::hint::black_box(loudness.short_term());
        }
    });
    let without = best_of(|| {
        for _ in 0..BLOCKS {
            fill(&mut buffer, &mut phase);
            std::hint::black_box(buffer.peak());
        }
    });
    (with - without) / BLOCKS as f64
}

fn report(label: &str, seconds: f64) {
    let deadline = FRAMES as f64 / f64::from(RATE);
    println!(
        "  {label:<22} {:>9.2} us/block  {:>11.1}x realtime  {:>7.3} % of one core",
        seconds * 1.0e6,
        deadline / seconds,
        seconds / deadline * 100.0
    );
}

#[test]
#[ignore = "a CPU baseline; it fails when the full chain needs more than half a block"]
fn every_effect_and_the_full_chain_run_faster_than_realtime() {
    println!(
        "pipewire-effects CPU baseline, {RATE} Hz, {CHANNELS} channels, {FRAMES} frames per block, best of {ROUNDS} rounds"
    );
    let baseline = fill_cost(FRAMES);
    println!(
        "  fill only (not processing)  {:>9.2} us/block",
        baseline * 1.0e6
    );
    for descriptor in EFFECTS {
        let seconds = seconds_per_block(chain(&[descriptor], FRAMES), FRAMES) - baseline;
        report(descriptor.id, seconds.max(0.0));
    }
    let every: Vec<&EffectDescriptor> = EFFECTS.iter().collect();
    let full = seconds_per_block(chain(&every, FRAMES), FRAMES) - baseline;
    report("all effects", full.max(0.0));
    report("spectrum analyser", analyser_cost(FRAMES).max(0.0));
    report("loudness meter", loudness_cost(FRAMES).max(0.0));
    let deadline = FRAMES as f64 / f64::from(RATE);
    assert!(
        full < deadline * 0.5,
        "the full chain costs {:.2} us against a {:.0} us deadline",
        full * 1.0e6,
        deadline * 1.0e6
    );
}
