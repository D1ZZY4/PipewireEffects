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
fn a_non_48_k_rate_runs_the_denoiser_and_the_gate() {
    for rate in [44_100, 96_000] {
        let params = ParamSet::new(specs());
        params.set_bool("vad", true).unwrap();
        params.set_f32("vad_threshold", 100.0).unwrap();
        let mut effect = NoiseReduction::new(params);
        effect.prepare(AudioFormat::new(rate, 2));
        let mut peak = 0.0f32;
        for block in 0..60 {
            let mut buf = buffer(FRAME, 0.5);
            effect.process(&mut buf);
            assert!(buf.channel(0).iter().all(|value| value.is_finite()));
            if block >= 50 {
                peak = peak.max(buf.peak());
            }
        }
        assert!(peak < 0.1, "{rate} Hz left {peak}");
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

#[test]
fn the_release_maps_to_the_reference_grace_period() {
    assert_eq!(release_frames(0.0), 0);
    assert_eq!(release_frames(20.0), 2);
    assert_eq!(release_frames(20_000.0), 2_000);
}

#[test]
fn the_gate_opens_on_a_voice_frame_and_holds_for_the_release() {
    let mut hold = 0;
    assert_eq!(gate_target(true, &mut hold, 0), 1.0);
    assert_eq!(gate_target(false, &mut hold, 0), 0.0);

    let mut hold = 0;
    assert_eq!(gate_target(true, &mut hold, 2), 1.0);
    assert_eq!(gate_target(false, &mut hold, 2), 1.0);
    assert_eq!(gate_target(false, &mut hold, 2), 1.0);
    assert_eq!(gate_target(false, &mut hold, 2), 0.0);
}

#[test]
fn the_release_time_controls_how_long_the_gate_stays_open() {
    let make = |release: f32| {
        let params = ParamSet::new(specs());
        params.set_bool("vad", true).unwrap();
        params.set_f32("vad_threshold", 100.0).unwrap();
        params.set_f32("release", release).unwrap();
        let mut effect = prepared(params);
        effect.channels[0].gate = 1.0;
        effect.channels[0].first = false;
        effect.channels[0].hold = release_frames(release);
        effect
    };
    let mut fast = make(0.0);
    let mut slow = make(20_000.0);
    for _ in 0..12 {
        let mut fast_buffer = buffer(FRAME, 0.0);
        fast.process(&mut fast_buffer);
        let mut slow_buffer = buffer(FRAME, 0.0);
        slow.process(&mut slow_buffer);
    }
    assert!(fast.channels[0].gate < 0.01, "{}", fast.channels[0].gate);
    assert!(slow.channels[0].gate > 0.9, "{}", slow.channels[0].gate);
}
