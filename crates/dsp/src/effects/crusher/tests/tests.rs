use super::*;

fn ramp(samples: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, samples);
    buffer.configure(2, samples).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = -1.0 + 2.0 * index as f32 / samples as f32;
        }
    }
    buffer
}

fn constant(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        buffer.channel_mut(channel).fill(amplitude);
    }
    buffer
}

fn effect(settings: &[(&str, f32)]) -> Crusher {
    let params = ParamSet::new(specs());
    for (key, value) in settings {
        params.set_f32(key, *value).unwrap();
    }
    let mut effect = Crusher::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

#[test]
fn sixteen_bits_at_full_rate_is_transparent() {
    let mut crusher = effect(&[
        ("bits", 16.0),
        ("mix", 100.0),
        ("samples", 1.0),
        ("anti_aliasing", 0.0),
    ]);
    let mut buffer = ramp(4096);
    let source: Vec<f32> = buffer.channel(0).to_vec();
    crusher.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(source) {
        assert!(
            (processed - original).abs() < 1.0e-4,
            "{processed} {original}"
        );
    }
}

#[test]
fn a_zero_mix_keeps_the_original_signal() {
    let mut crusher = effect(&[("bits", 1.0), ("mix", 0.0), ("anti_aliasing", 100.0)]);
    let mut buffer = ramp(1024);
    let source: Vec<f32> = buffer.channel(0).to_vec();
    crusher.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(source) {
        assert!(
            (processed - original).abs() < 1.0e-6,
            "{processed} {original}"
        );
    }
}

#[test]
fn two_bits_quantises_to_linear_steps() {
    let mut crusher = effect(&[
        ("bits", 2.0),
        ("mix", 100.0),
        ("samples", 1.0),
        ("anti_aliasing", 0.0),
    ]);
    let mut buffer = ramp(2048);
    crusher.process(&mut buffer);
    for sample in buffer.channel(0) {
        let steps = sample / 0.5;
        assert!((steps - steps.round()).abs() < 1.0e-4, "{sample}");
        assert!(sample.abs() <= 1.0 + 1.0e-4);
    }
}

#[test]
fn the_logarithmic_mode_does_not_gate_low_levels() {
    let mut linear = effect(&[("bits", 4.0), ("mix", 100.0), ("anti_aliasing", 0.0)]);
    let mut logarithmic = effect(&[
        ("bits", 4.0),
        ("mix", 100.0),
        ("mode", 1.0),
        ("anti_aliasing", 0.0),
    ]);
    let mut quiet = constant(0.01, 64);
    let mut other = constant(0.01, 64);
    linear.process(&mut quiet);
    logarithmic.process(&mut other);
    assert_eq!(quiet.peak(), 0.0);
    assert!(other.peak() > 0.001, "{}", other.peak());
}

#[test]
fn sample_reduction_holds_each_value_for_the_factor() {
    let mut crusher = effect(&[
        ("bits", 16.0),
        ("mix", 100.0),
        ("samples", 8.0),
        ("anti_aliasing", 0.0),
    ]);
    let mut buffer = ramp(2048);
    crusher.process(&mut buffer);
    let plan = buffer.channel(0);
    for block in plan.chunks(8) {
        assert!(
            block.iter().all(|value| (value - block[0]).abs() < 1.0e-6),
            "{block:?}"
        );
    }
}

#[test]
fn modulation_moves_the_reduction_factor() {
    let mut crusher = effect(&[
        ("bits", 16.0),
        ("mix", 100.0),
        ("samples", 1.0),
        ("anti_aliasing", 0.0),
        ("lfo_active", 1.0),
        ("lfo_depth", 40.0),
        ("lfo_rate", 20.0),
    ]);
    let mut buffer = ramp(8192);
    crusher.process(&mut buffer);
    let repeats = buffer
        .channel(0)
        .windows(2)
        .filter(|pair| (pair[0] - pair[1]).abs() < 1.0e-9)
        .count();
    assert!(repeats > 0, "the modulated crusher never held a sample");
}

#[test]
fn abusive_settings_stay_finite_and_bounded() {
    let mut crusher = effect(&[
        ("bits", 1.0),
        ("samples", 250.0),
        ("anti_aliasing", 100.0),
        ("dc", 12.041),
        ("mix", 100.0),
    ]);
    let mut buffer = ramp(4096);
    crusher.process(&mut buffer);
    for sample in buffer.channel(0) {
        assert!(sample.is_finite(), "{sample}");
        assert!(sample.abs() <= 5.0, "{sample}");
    }
}
