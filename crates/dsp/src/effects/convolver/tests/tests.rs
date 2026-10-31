use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet};

use super::impulse;
use super::{Convolver, partitioned::BLOCK, specs as convolver_specs};

fn configured(wet: f32, dry: f32, auto_gain: bool, response: u16) -> Convolver {
    let params = ParamSet::new(convolver_specs());
    params.set_f32("wet", wet).unwrap();
    params.set_f32("dry", dry).unwrap();
    params
        .set_choice("auto_gain", u16::from(auto_gain))
        .unwrap();
    params.set_choice("response", response).unwrap();
    let mut effect = Convolver::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

fn impulse_buffer(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer.channel_mut(0)[0] = 1.0;
    buffer
}

fn silent(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer
}

#[test]
fn a_wet_impulse_reproduces_the_selected_response_one_block_late() {
    let mut effect = configured(0.0, -60.0, true, 0);
    let mut buffer = impulse_buffer(1024);
    effect.process(&mut buffer);
    let expected = impulse::normalised(0, 48_000, 0, true);
    for (index, wanted) in expected.iter().enumerate().take(256) {
        let actual = buffer.channel(0)[BLOCK + index];
        assert!(
            (actual - wanted).abs() < 1.0e-4,
            "sample {index}: {actual} against {wanted}"
        );
    }
}

#[test]
fn a_dry_setting_passes_the_input_through() {
    let mut effect = configured(-60.0, 0.0, true, 0);
    let mut buffer = impulse_buffer(512);
    buffer.channel_mut(1)[3] = 0.5;
    let reference = buffer.channel(0).to_vec();
    effect.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
        assert!((processed - original).abs() < 1.0e-2);
    }
}

#[test]
fn both_sides_at_the_floor_stay_quiet() {
    let mut effect = configured(-60.0, -60.0, true, 0);
    let mut buffer = impulse_buffer(2048);
    effect.process(&mut buffer);
    assert!(buffer.peak() < 2.0e-3, "{}", buffer.peak());
}

#[test]
fn auto_gain_preserves_the_signal_energy() {
    let mut effect = configured(0.0, -60.0, true, 0);
    let mut buffer = impulse_buffer(4096);
    effect.process(&mut buffer);
    let energy: f32 = buffer.channel(0).iter().map(|value| value * value).sum();
    assert!(
        (energy - 1.0).abs() < 0.05,
        "the wet impulse carried {energy} of energy"
    );
}

#[test]
fn without_auto_gain_the_response_keeps_its_recorded_peak() {
    let mut effect = configured(0.0, -60.0, false, 0);
    let mut buffer = impulse_buffer(1024);
    effect.process(&mut buffer);
    let peak = buffer
        .channel(0)
        .iter()
        .fold(0.0f32, |current, value| current.max(value.abs()));
    assert!((peak - 0.5).abs() < 1.0e-3, "the wet peak was {peak}");
}

#[test]
fn each_response_produces_a_different_tail() {
    let tail = |response: u16| {
        let mut effect = configured(0.0, -60.0, true, response);
        let mut buffer = impulse_buffer(4096);
        effect.process(&mut buffer);
        buffer.channel(0).to_vec()
    };
    let small = tail(0);
    let hall = tail(2);
    let difference: f32 = small
        .iter()
        .zip(hall.iter())
        .map(|(left, right)| (left - right).abs())
        .sum();
    assert!(difference > 0.01, "the two responses were the same");
}

#[test]
fn a_long_tail_outlives_the_first_block() {
    let mut effect = configured(0.0, -60.0, true, 2);
    let mut buffer = impulse_buffer(BLOCK);
    effect.process(&mut buffer);
    let mut energy = 0.0f32;
    for _ in 0..4 {
        let mut block = silent(BLOCK);
        effect.process(&mut block);
        energy += block
            .channel(0)
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
    }
    assert!(energy > 1.0e-4, "the tail energy was {energy}");
}

#[test]
fn output_stays_finite_with_extreme_settings() {
    let mut effect = configured(0.0, 0.0, false, 2);
    for _ in 0..64 {
        let mut buffer = impulse_buffer(1024);
        for (index, sample) in buffer.channel_mut(0).iter_mut().enumerate() {
            *sample = if index % 2 == 0 { 1.0 } else { -0.9 };
        }
        for (index, sample) in buffer.channel_mut(1).iter_mut().enumerate() {
            *sample = if index % 3 == 0 { 0.7 } else { -0.4 };
        }
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 32.0, "{sample}");
            }
        }
    }
}
