use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet};

use super::{EchoCanceller, specs as echo_specs};

const RATE: u32 = 48_000;
const ECHO_DELAY: usize = 96;
const BLOCK: usize = 512;
const RING: usize = 1 << 14;

fn configured(mix: f32, adaptation: f32, delay: f32) -> EchoCanceller {
    let params = ParamSet::new(echo_specs());
    params.set_f32("mix", mix).unwrap();
    params.set_f32("adaptation", adaptation).unwrap();
    params.set_f32("delay", delay).unwrap();
    params.set_f32("filter_length", 40.0).unwrap();
    let mut effect = EchoCanceller::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    effect
}

fn configured_with(settings: &[(&str, f32)]) -> EchoCanceller {
    let params = ParamSet::new(echo_specs());
    for (id, value) in settings {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = EchoCanceller::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    effect
}

fn noise(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn run(effect: &mut EchoCanceller, blocks: usize) -> (f32, f32) {
    scene(effect, blocks, ECHO_DELAY, 0.0)
}

fn scene(effect: &mut EchoCanceller, blocks: usize, echo_delay: usize, speech: f32) -> (f32, f32) {
    let mut history = vec![0.0f32; RING];
    let mut cursor = 0usize;
    let mut far_state = 0x1234_5678u32;
    let mut speech_state = 0x0BAD_F00Du32;
    let mut near_energy = 0.0f64;
    let mut out_energy = 0.0f64;
    for block in 0..blocks {
        let mut buffer = AudioBuffer::new(2, BLOCK);
        buffer.configure(2, BLOCK).unwrap();
        let mut reference = vec![0.0f32; BLOCK * 2];
        for frame in 0..BLOCK {
            let far = noise(&mut far_state) * 0.25;
            history[cursor] = far;
            let first = history[(cursor + RING - echo_delay) % RING];
            let second = history[(cursor + RING - echo_delay - 31) % RING];
            cursor = (cursor + 1) % RING;
            let echo = 0.55 * first + 0.2 * second;
            let talk = noise(&mut speech_state) * speech;
            for channel in 0..2 {
                buffer.channel_mut(channel)[frame] = echo + talk;
                reference[frame * 2 + channel] = far;
            }
            if block >= blocks.saturating_sub(30) {
                near_energy += 2.0 * f64::from(echo) * f64::from(echo);
            }
        }
        effect.process_with_reference(&mut buffer, &reference);
        if block >= blocks.saturating_sub(30) {
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    out_energy += f64::from(*sample) * f64::from(*sample);
                }
            }
        }
    }
    (near_energy as f32, out_energy as f32)
}

fn weight_change(before: &[Vec<f32>; 2], after: &[Vec<f32>; 2]) -> f32 {
    let mut sum = 0.0f32;
    for channel in 0..2 {
        for (old, new) in before[channel].iter().zip(after[channel].iter()) {
            let delta = new - old;
            sum += delta * delta;
        }
    }
    sum
}

fn sine(amplitude: f32, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, BLOCK);
    buffer.configure(2, BLOCK).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = std::f32::consts::TAU * 220.0 * (offset + index) as f32 / RATE as f32;
            *sample = amplitude * phase.sin();
        }
    }
    buffer
}

#[test]
fn an_echo_is_cancelled_after_the_filter_adapts() {
    let mut effect = configured(100.0, 60.0, 2.0);
    let (reference, output) = run(&mut effect, 240);
    assert!(
        output < reference * 0.2,
        "the output carried {output} against {reference} of echo reference"
    );
}

#[test]
fn a_zero_mix_leaves_the_signal_untouched() {
    let mut effect = configured(0.0, 60.0, 2.0);
    let mut buffer = AudioBuffer::new(2, 256);
    buffer.configure(2, 256).unwrap();
    for frame in 0..256 {
        buffer.channel_mut(0)[frame] = frame as f32 / 256.0;
        buffer.channel_mut(1)[frame] = -(frame as f32) / 256.0;
    }
    let reference = vec![0.3f32; 512];
    let expected = buffer.channel(0).to_vec();
    effect.process_with_reference(&mut buffer, &reference);
    for (actual, wanted) in buffer.channel(0).iter().zip(expected.iter()) {
        assert!((actual - wanted).abs() < 1.0e-6);
    }
}

#[test]
fn without_a_reference_the_stage_is_transparent() {
    let mut effect = configured(100.0, 60.0, 2.0);
    let mut buffer = AudioBuffer::new(2, 128);
    buffer.configure(2, 128).unwrap();
    for frame in 0..128 {
        buffer.channel_mut(0)[frame] = 0.25;
    }
    effect.process(&mut buffer);
    assert!(
        buffer
            .channel(0)
            .iter()
            .all(|value| (*value - 0.25).abs() < 1.0e-6)
    );
}

#[test]
fn adaptation_off_does_not_change_the_near_signal() {
    let mut effect = configured(100.0, 0.0, 2.0);
    let reference = vec![0.5f32; 512];
    for _ in 0..32 {
        let mut buffer = AudioBuffer::new(2, 256);
        buffer.configure(2, 256).unwrap();
        buffer.channel_mut(0)[0] = 1.0;
        buffer.channel_mut(1)[1] = -0.5;
        effect.process_with_reference(&mut buffer, &reference);
        assert!((buffer.channel(0)[0] - 1.0).abs() < 1.0e-6);
        assert!((buffer.channel(1)[1] + 0.5).abs() < 1.0e-6);
    }
}

#[test]
fn output_stays_finite_with_extreme_settings() {
    let mut effect = configured(100.0, 100.0, 500.0);
    let mut state = 0xABCD_EF01u32;
    for _ in 0..300 {
        let mut buffer = AudioBuffer::new(2, 512);
        buffer.configure(2, 512).unwrap();
        let mut reference = vec![0.0f32; 1_024];
        for frame in 0..512 {
            for channel in 0..2 {
                let value = noise(&mut state) * 2.0;
                buffer.channel_mut(channel)[frame] = value;
                reference[frame * 2 + channel] = value;
            }
        }
        effect.process_with_reference(&mut buffer, &reference);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 64.0, "{sample}");
            }
        }
    }
}

#[test]
fn automatic_delay_alignment_cancels_an_echo_beyond_the_manual_window() {
    let mut effect = configured_with(&[
        ("delay", 0.0),
        ("auto_delay", 1.0),
        ("filter_length", 40.0),
        ("adaptation", 60.0),
        ("mix", 100.0),
    ]);
    let (reference, output) = scene(&mut effect, 600, 2_880, 0.0);
    assert!(
        output < reference * 0.25,
        "the output carried {output} against {reference}"
    );
    assert!(effect.estimator.has_estimate());
    let found = effect.estimator.delay_frames() as isize;
    assert!((found - 2_880).abs() <= 96, "the estimator found {found}");
}

#[test]
fn a_high_pass_filter_removes_the_direct_current() {
    let mut effect = configured_with(&[
        ("high_pass", 1.0),
        ("high_pass_cutoff", 80.0),
        ("adaptation", 0.0),
        ("mix", 100.0),
    ]);
    let mut buffer = AudioBuffer::new(2, BLOCK);
    buffer.configure(2, BLOCK).unwrap();
    let reference = vec![0.5f32; BLOCK * 2];
    for _ in 0..40 {
        buffer.channel_mut(0).fill(0.5);
        buffer.channel_mut(1).fill(-0.5);
        effect.process_with_reference(&mut buffer, &reference);
    }
    assert!(
        buffer.peak() < 0.05,
        "the direct current survived at {}",
        buffer.peak()
    );
}

#[test]
fn the_automatic_gain_lifts_a_quiet_signal() {
    let mut effect = configured_with(&[("agc", 1.0), ("adaptation", 0.0), ("mix", 100.0)]);
    let reference = vec![0.0f32; BLOCK * 2];
    let mut buffer = sine(0.01, 0);
    for block in 0..200 {
        buffer = sine(0.01, block * BLOCK);
        effect.process_with_reference(&mut buffer, &reference);
    }
    assert!(
        buffer.peak() > 0.02,
        "the gain reached only {}",
        buffer.peak()
    );
}

#[test]
fn the_double_talk_detector_freezes_adaptation() {
    let mut frozen = configured_with(&[
        ("delay", 2.0),
        ("filter_length", 40.0),
        ("adaptation", 60.0),
        ("double_talk", 1.0),
    ]);
    let mut open = configured_with(&[
        ("delay", 2.0),
        ("filter_length", 40.0),
        ("adaptation", 60.0),
    ]);
    scene(&mut frozen, 200, ECHO_DELAY, 0.0);
    scene(&mut open, 200, ECHO_DELAY, 0.0);
    let frozen_before = frozen.weights.clone();
    let open_before = open.weights.clone();
    scene(&mut frozen, 40, ECHO_DELAY, 0.4);
    scene(&mut open, 40, ECHO_DELAY, 0.4);
    let frozen_change = weight_change(&frozen_before, &frozen.weights);
    let open_change = weight_change(&open_before, &open.weights);
    assert!(frozen.double_talk_active, "the detector did not engage");
    assert!(
        frozen_change < open_change * 0.5,
        "frozen {frozen_change}, open {open_change}"
    );
}

#[test]
fn the_mobile_mode_caps_the_filter_and_doubles_the_step() {
    let fixed = configured_with(&[("filter_length", 40.0)]);
    let mobile = configured_with(&[("filter_length", 40.0), ("mode", 1.0)]);
    assert_eq!(fixed.step_scale(), 1.0);
    assert_eq!(mobile.step_scale(), 2.0);
    assert_eq!(fixed.effective_taps(), 1_920);
    assert_eq!(mobile.effective_taps(), 960);
}
