use std::process::ExitCode;
use std::time::Instant;

use pwe_core::{AudioBuffer, AudioFormat, EffectChain, ParamSet};
use pwe_dsp::{EFFECTS, EffectDescriptor};

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;

fn fill(buffer: &mut AudioBuffer, phase: &mut f32) {
    let step = std::f32::consts::TAU * 997.0 / RATE as f32;
    let frames = buffer.frames();
    for frame in 0..frames {
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

fn sound(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(CHANNELS, frames);
    if buffer.configure(CHANNELS, frames).is_err() {
        eprintln!("the buffer rejected {frames} frames");
        std::process::exit(1);
    }
    buffer
}

fn chain() -> EffectChain {
    let mut chain = EffectChain::new(EFFECTS.len(), AudioFormat::new(RATE, CHANNELS));
    for descriptor in EFFECTS {
        if chain.push(build(descriptor)).is_err() {
            eprintln!("the chain rejected {}", descriptor.id);
        }
    }
    chain
}

fn build(descriptor: &EffectDescriptor) -> Box<dyn pwe_core::Effect> {
    let params = ParamSet::new(descriptor.specs());
    (descriptor.build)(params)
}

fn one(descriptor: &EffectDescriptor) -> EffectChain {
    let mut chain = EffectChain::new(1, AudioFormat::new(RATE, CHANNELS));
    if chain.push(build(descriptor)).is_err() {
        eprintln!("the chain rejected {}", descriptor.id);
    }
    chain
}

fn seconds_per_block(mut chain: EffectChain, frames: usize, blocks: usize) -> f64 {
    let mut buffer = sound(frames);
    let mut phase = 0.0;
    for _ in 0..64 {
        fill(&mut buffer, &mut phase);
        chain.process(&mut buffer);
    }
    let start = Instant::now();
    for _ in 0..blocks {
        fill(&mut buffer, &mut phase);
        chain.process(&mut buffer);
    }
    start.elapsed().as_secs_f64() / blocks as f64
}

fn fill_cost(frames: usize, blocks: usize) -> f64 {
    let mut buffer = sound(frames);
    let mut phase = 0.0;
    let start = Instant::now();
    for _ in 0..blocks {
        fill(&mut buffer, &mut phase);
    }
    let seconds = start.elapsed().as_secs_f64() / blocks as f64;
    println!(
        "  fill checksum {:.4} (keeps the measurement observable)",
        buffer.peak()
    );
    seconds
}

fn report(label: &str, seconds: f64, frames: usize) {
    let deadline = frames as f64 / f64::from(RATE);
    let realtime = deadline / seconds;
    let cpu = seconds / deadline * 100.0;
    println!(
        "  {label:<22} {:>9.2} us/block  {:>11.1}x realtime  {:>7.3} % of one core",
        seconds * 1.0e6,
        realtime,
        cpu
    );
}

fn sweep(frames: usize, blocks: usize, baseline: f64) {
    println!("frames per block {frames}, {blocks} blocks");
    for descriptor in EFFECTS {
        let seconds = seconds_per_block(one(descriptor), frames, blocks) - baseline;
        report(descriptor.id, seconds.max(0.0), frames);
    }
    let all = seconds_per_block(chain(), frames, blocks) - baseline;
    report("all twenty-three", all.max(0.0), frames);
}

fn main() -> ExitCode {
    println!(
        "pipewire-effects CPU baseline, {} Hz, {} channels",
        RATE, CHANNELS
    );
    let baseline = fill_cost(1024, 2_000);
    println!(
        "  fill only (not processing)  {:>9.2} us/block",
        baseline * 1.0e6
    );
    sweep(1024, 2_000, baseline);
    sweep(128, 2_000, fill_cost(128, 2_000));
    ExitCode::SUCCESS
}
