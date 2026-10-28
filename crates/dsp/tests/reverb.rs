use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet};
use pwe_dsp::effects::{Reverb, reverb_specs};

fn burst(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            if index < 64 {
                *sample = (std::f32::consts::TAU * 440.0 * index as f32 / 48_000.0).sin();
            }
        }
    }
    buffer
}

fn silent(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer
}

fn tail_energy(effect: &mut Reverb, blocks: usize) -> f32 {
    let mut energy = 0.0f32;
    for _ in 0..blocks {
        let mut buffer = silent(1024);
        effect.process(&mut buffer);
        energy += buffer.peak();
    }
    energy
}

fn reverb() -> Reverb {
    let mut effect = Reverb::new(ParamSet::new(reverb_specs()));
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

#[test]
fn a_burst_leaves_an_audible_tail() {
    let mut effect = reverb();
    let mut buffer = burst(2048);
    effect.process(&mut buffer);
    let energy = tail_energy(&mut effect, 200);
    assert!(energy > 0.01, "tail energy {energy}");
}

#[test]
fn the_tail_decays_over_time() {
    let params = ParamSet::new(reverb_specs());
    params.set_f32("room_size", 30.0).unwrap();
    let mut effect = Reverb::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = burst(2048);
    effect.process(&mut buffer);
    let early = tail_energy(&mut effect, 40);
    let late = tail_energy(&mut effect, 40);
    assert!(late < early, "early {early} late {late}");
}

#[test]
fn a_completely_dry_setting_is_transparent() {
    let params = ParamSet::new(reverb_specs());
    params.set_f32("wet", -60.0).unwrap();
    let mut effect = Reverb::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = burst(512);
    let reference = buffer.channel(0).to_vec();
    for _ in 0..8 {
        effect.process(&mut buffer);
    }
    for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
        assert!((processed - original).abs() < 1.0e-3);
    }
}

#[test]
fn re_tuning_keeps_the_decay_time_stable() {
    let mut effect = reverb();
    effect.prepare(AudioFormat::new(96_000, 2));
    let mut buffer = burst(2048);
    effect.process(&mut buffer);
    let energy = tail_energy(&mut effect, 200);
    assert!(energy > 0.01, "tail energy at 96 kHz: {energy}");
}

#[test]
fn output_stays_finite_with_extreme_settings() {
    let params = ParamSet::new(reverb_specs());
    params.set_f32("room_size", 100.0).unwrap();
    params.set_f32("wet", 0.0).unwrap();
    let mut effect = Reverb::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    for _ in 0..300 {
        let mut buffer = burst(1024);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 8.0, "{sample}");
            }
        }
    }
}
