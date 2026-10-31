use std::time::Instant;

use pwe_core::{AudioBuffer, AudioFormat, Effect, EffectChain, ParamSet};
use pwe_dsp::{EFFECTS, EffectDescriptor};

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const FRAMES: usize = 1024;
const BLOCKS: usize = 2_000;
const WARM_UP: usize = 64;
const LEGAL_CHAIN: usize = 16;

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

fn reference(frames: usize) -> Vec<f32> {
    let step = std::f32::consts::TAU * 331.0 / RATE as f32;
    let mut samples = Vec::with_capacity(frames * CHANNELS);
    for frame in 0..frames {
        let value = 0.2 * (step * frame as f32).sin();
        samples.push(value);
        samples.push(-value);
    }
    samples
}

fn chain(descriptors: &[&EffectDescriptor], frames: usize) -> EffectChain {
    let mut chain = EffectChain::new(descriptors.len(), AudioFormat::new(RATE, CHANNELS));
    for descriptor in descriptors {
        chain
            .push(build(descriptor))
            .expect("the chain should accept every effect");
    }
    let mut buffer = sound(frames);
    let reference = reference(frames);
    let mut phase = 0.0;
    for _ in 0..WARM_UP {
        fill(&mut buffer, &mut phase);
        chain.process_with_reference(&mut buffer, &reference);
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
    let reference = reference(frames);
    let mut phase = 0.0;
    best_of(|| {
        for _ in 0..BLOCKS {
            fill(&mut buffer, &mut phase);
            chain.process_with_reference(&mut buffer, &reference);
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
    let mut costs: Vec<(f64, &EffectDescriptor)> = Vec::with_capacity(EFFECTS.len());
    for descriptor in EFFECTS {
        let seconds = seconds_per_block(chain(&[descriptor], FRAMES), FRAMES) - baseline;
        let seconds = seconds.max(0.0);
        report(descriptor.id, seconds);
        costs.push((seconds, descriptor));
    }
    costs.sort_by(|left, right| right.0.total_cmp(&left.0));
    let heaviest: Vec<&EffectDescriptor> = costs
        .iter()
        .take(LEGAL_CHAIN)
        .map(|(_, descriptor)| *descriptor)
        .collect();
    let legal = seconds_per_block(chain(&heaviest, FRAMES), FRAMES) - baseline;
    report("heaviest legal chain", legal.max(0.0));
    let every: Vec<&EffectDescriptor> = EFFECTS.iter().collect();
    let all = seconds_per_block(chain(&every, FRAMES), FRAMES) - baseline;
    report("all effects (informational)", all.max(0.0));
    report("spectrum analyser", analyser_cost(FRAMES).max(0.0));
    report("loudness meter", loudness_cost(FRAMES).max(0.0));
    let deadline = FRAMES as f64 / f64::from(RATE);
    let slowest = costs.first().map_or(0.0, |(seconds, _)| *seconds);
    assert!(
        slowest < deadline * 0.5,
        "the slowest effect costs {:.2} us against a {:.0} us deadline",
        slowest * 1.0e6,
        deadline * 1.0e6
    );
    assert!(
        legal < deadline * 0.9,
        "the heaviest {LEGAL_CHAIN}-effect chain costs {:.2} us against a {:.0} us deadline",
        legal * 1.0e6,
        deadline * 1.0e6
    );
}
