use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn prepared(params: ParamSet) -> Limiter {
    let mut effect = Limiter::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

#[test]
fn the_ceiling_is_never_exceeded() {
    let params = ParamSet::new(specs());
    params.set_f32("input_gain", 20.0).unwrap();
    params.set_f32("limit", -6.0).unwrap();
    let mut effect = prepared(params);
    let ceiling = math::db_to_gain(-6.0);
    for _ in 0..128 {
        let mut buffer = tone(1.0, 1024);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.abs() <= ceiling + 1.0e-6, "{sample} > {ceiling}");
            }
        }
    }
}

#[test]
fn a_limited_burst_keeps_its_shape_instead_of_being_clipped() {
    let params = ParamSet::new(specs());
    params.set_f32("limit", -6.0).unwrap();
    params.set_f32("lookahead", 20.0).unwrap();
    let mut effect = prepared(params);
    let ceiling = math::db_to_gain(-6.0);
    for _ in 0..4 {
        let mut buffer = tone(1.0, 8192);
        effect.process(&mut buffer);
        let steady = &buffer.channel(0)[4096..];
        let peak = steady.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let rms = (steady.iter().map(|v| v * v).sum::<f32>() / steady.len() as f32).sqrt();
        let crest = peak / rms;
        assert!(
            (crest - std::f32::consts::SQRT_2).abs() < 0.03,
            "the burst was clipped: its crest factor is {crest} instead of {}",
            std::f32::consts::SQRT_2
        );
        assert!((peak - ceiling).abs() < 0.01, "{peak} is not at {ceiling}");
    }
}

#[test]
fn quiet_signal_passes_through_unity() {
    let params = ParamSet::new(specs());
    let mut effect = prepared(params);
    for _ in 0..4 {
        let mut buffer = tone(0.05, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.05).abs() < 0.005, "{}", buffer.peak());
    }
}

#[test]
fn the_output_lags_by_the_lookahead() {
    let params = ParamSet::new(specs());
    params.set_f32("lookahead", 10.0).unwrap();
    let effect = prepared(params);
    assert_eq!(effect.latency_frames(), 480);
    let mut impulse = AudioBuffer::new(2, 1024);
    impulse.configure(2, 1024).unwrap();
    impulse.channel_mut(0)[0] = 0.25;
    impulse.channel_mut(1)[0] = 0.25;
    let mut effect = effect;
    effect.process(&mut impulse);
    assert!(impulse.channel(0)[..480].iter().all(|value| *value == 0.0));
    assert!((impulse.channel(0)[480] - 0.25).abs() < 1.0e-6);
}

#[test]
fn the_gain_recovers_after_a_transient() {
    let params = ParamSet::new(specs());
    params.set_f32("limit", -12.0).unwrap();
    params.set_f32("release", 5.0).unwrap();
    let mut effect = prepared(params);
    let mut burst = tone(1.0, 256);
    effect.process(&mut burst);
    let mut flush = AudioBuffer::new(2, 4096);
    flush.configure(2, 4096).unwrap();
    effect.process(&mut flush);
    let mut quiet = tone(0.02, 4096);
    for _ in 0..64 {
        effect.process(&mut quiet);
    }
    assert!((quiet.peak() - 0.02).abs() < 0.002, "{}", quiet.peak());
}

#[test]
fn the_stereo_link_blends_the_channels() {
    let params = ParamSet::new(specs());
    params.set_f32("limit", -6.0).unwrap();
    params.set_f32("stereo_link", 0.0).unwrap();
    params.set_f32("lookahead", 1.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = AudioBuffer::new(2, 2048);
    buffer.configure(2, 2048).unwrap();
    for (index, sample) in buffer.channel_mut(0).iter_mut().enumerate() {
        *sample = 0.9 * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
    }
    for (index, sample) in buffer.channel_mut(1).iter_mut().enumerate() {
        *sample = 0.01 * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
    }
    effect.process(&mut buffer);
    let left = buffer
        .channel(0)
        .iter()
        .skip(64)
        .fold(0.0f32, |a, b| a.max(b.abs()));
    let right = buffer
        .channel(1)
        .iter()
        .skip(64)
        .fold(0.0f32, |a, b| a.max(b.abs()));
    assert!(left < 0.51, "the loud channel was not limited: {left}");
    assert!(
        right > 0.009,
        "the quiet channel was limited with it: {right}"
    );
}
