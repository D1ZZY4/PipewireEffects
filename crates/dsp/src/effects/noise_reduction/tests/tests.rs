use super::*;

fn buffer(frames: usize, value: f32) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer.channel_mut(0).fill(value);
    buffer.channel_mut(1).fill(value);
    buffer
}

fn prepared(params: ParamSet) -> NoiseReduction {
    let mut effect = NoiseReduction::new(params);
    effect.prepare(AudioFormat::new(SUPPORTED_RATE, 2));
    effect
}

#[test]
fn an_unsupported_rate_passes_the_signal_through() {
    let params = ParamSet::new(specs());
    params.set_f32("input_gain", 0.0).unwrap();
    params.set_f32("output_gain", 0.0).unwrap();
    let mut effect = NoiseReduction::new(params);
    effect.prepare(AudioFormat::new(44_100, 2));
    let mut buf = buffer(512, 0.25);
    effect.process(&mut buf);
    for sample in buf.channel(0) {
        assert!((sample - 0.25).abs() < 1.0e-6);
    }
}

#[test]
fn the_output_lags_by_one_frame_and_the_first_frame_is_the_dry_input() {
    let params = ParamSet::new(specs());
    let mut effect = prepared(params);
    let mut output = Vec::new();
    for _ in 0..6 {
        let mut buf = buffer(FRAME, 0.5);
        effect.process(&mut buf);
        output.extend_from_slice(buf.channel(0));
    }
    assert_eq!(output.len(), 6 * FRAME);
    assert!(output[..FRAME - 1].iter().all(|value| value.abs() < 1.0e-6));
    assert!((output[FRAME - 1] - 0.5).abs() < 1.0e-6);
    assert!(output.iter().all(|value| value.is_finite()));
}

#[test]
fn a_zero_mix_leaves_the_signal_untouched() {
    let params = ParamSet::new(specs());
    params.set_f32("mix", 0.0).unwrap();
    let mut effect = prepared(params);
    let mut buf = buffer(FRAME, 0.5);
    effect.process(&mut buf);
    for sample in buf.channel(0) {
        assert!((sample - 0.5).abs() < 1.0e-6);
    }
}

#[test]
fn output_gain_scales_the_result() {
    let params = ParamSet::new(specs());
    params.set_f32("mix", 0.0).unwrap();
    params.set_f32("output_gain", -6.0206).unwrap();
    let mut effect = prepared(params);
    let mut buf = buffer(FRAME, 0.5);
    effect.process(&mut buf);
    assert!((buf.channel(0)[0] - 0.25).abs() < 1.0e-3);
}

#[test]
fn the_elements_are_finite_under_abuse() {
    let params = ParamSet::new(specs());
    params.set_f32("input_gain", 36.0).unwrap();
    params.set_f32("output_gain", 36.0).unwrap();
    params.set_bool("vad", true).unwrap();
    params.set_f32("vad_threshold", 100.0).unwrap();
    let mut effect = prepared(params);
    for _ in 0..4 {
        let mut buf = buffer(FRAME, 0.9);
        effect.process(&mut buf);
        assert!(buf.channel(0).iter().all(|value| value.is_finite()));
    }
}
