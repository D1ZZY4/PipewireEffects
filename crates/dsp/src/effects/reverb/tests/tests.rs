use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet};

use super::{Reverb, specs as reverb_specs};

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
fn the_tone_filters_shape_the_wet_signal() {
    let wide = {
        let params = ParamSet::new(reverb_specs());
        params.set_f32("bass_cut", 20.0).unwrap();
        params.set_f32("treble_cut", 20_000.0).unwrap();
        let mut effect = Reverb::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = burst(2048);
        effect.process(&mut buffer);
        tail_energy(&mut effect, 200)
    };
    let narrowed = {
        let params = ParamSet::new(reverb_specs());
        params.set_f32("bass_cut", 1000.0).unwrap();
        params.set_f32("treble_cut", 1500.0).unwrap();
        let mut effect = Reverb::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = burst(2048);
        effect.process(&mut buffer);
        tail_energy(&mut effect, 200)
    };
    assert!(
        narrowed < wide * 0.9,
        "the tone filters did not narrow the tail: {narrowed} against {wide}"
    );
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
fn the_pre_delay_does_not_leak_one_channel_into_the_other() {
    let params = ParamSet::new(reverb_specs());
    params.set_f32("pre_delay", 40.0).unwrap();
    params.set_f32("dry", -60.0).unwrap();
    params.set_f32("wet", 0.0).unwrap();
    params.set_f32("width", 100.0).unwrap();
    params.set_f32("bass_cut", 20.0).unwrap();
    params.set_f32("treble_cut", 20_000.0).unwrap();
    let mut effect = Reverb::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));

    let mut impulse = AudioBuffer::new(2, 1);
    impulse.configure(2, 1).unwrap();
    impulse.channel_mut(1)[0] = 1.0;
    effect.process(&mut impulse);

    let mut left = 0.0f32;
    let mut right = 0.0f32;
    for _ in 0..200 {
        let mut block = silent(1024);
        effect.process(&mut block);
        left = left.max(block.channel(0).iter().fold(0.0f32, |m, s| m.max(s.abs())));
        right = right.max(block.channel(1).iter().fold(0.0f32, |m, s| m.max(s.abs())));
    }
    assert!(right > 0.01, "the right channel produced no tail: {right}");
    assert!(
        left < 1.0e-4,
        "the right channel leaked into the left: {left}"
    );
}

#[test]
fn each_character_tunes_the_tank_differently() {
    let mut small = {
        let params = ParamSet::new(reverb_specs());
        params.set_choice("character", 0).unwrap();
        let mut effect = Reverb::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        effect
    };
    let mut tunnel = {
        let params = ParamSet::new(reverb_specs());
        params.set_choice("character", 3).unwrap();
        let mut effect = Reverb::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        effect
    };
    let mut first = burst(2048);
    let mut second = burst(2048);
    small.process(&mut first);
    tunnel.process(&mut second);
    let mut difference = 0.0f32;
    for _ in 0..20 {
        let mut a = silent(1024);
        let mut b = silent(1024);
        small.process(&mut a);
        tunnel.process(&mut b);
        for (left, right) in a.channel(0).iter().zip(b.channel(0)) {
            difference += (left - right).abs();
        }
    }
    assert!(
        difference > 0.1,
        "two characters produced the same tail: {difference}"
    );
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

#[test]
fn the_tank_mix_control_is_published_and_ranged() {
    let specs = reverb_specs();
    let spec = specs
        .iter()
        .find(|spec| spec.id == "tank_mix")
        .expect("tank_mix is missing");
    assert_eq!(spec.group, "Room");
    assert_eq!(spec.default_value().as_f32(), 100.0);
}

#[test]
fn a_smaller_tank_mix_changes_the_tail() {
    let mut dry_tank = {
        let params = ParamSet::new(reverb_specs());
        params.set_f32("tank_mix", 0.0).unwrap();
        let mut effect = Reverb::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        effect
    };
    let mut full_tank = reverb();
    let mut first = burst(2048);
    let mut second = burst(2048);
    dry_tank.process(&mut first);
    full_tank.process(&mut second);
    let mut difference = 0.0f32;
    for _ in 0..40 {
        let mut a = silent(1024);
        let mut b = silent(1024);
        dry_tank.process(&mut a);
        full_tank.process(&mut b);
        for (left, right) in a.channel(0).iter().zip(b.channel(0)) {
            difference += (left - right).abs();
        }
    }
    assert!(difference > 0.05, "the tank mix did nothing: {difference}");
}
