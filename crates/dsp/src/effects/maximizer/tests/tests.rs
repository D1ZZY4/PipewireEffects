use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 500.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn settle(threshold: f32, amplitude: f32) -> f32 {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", threshold).unwrap();
    let mut effect = Maximizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    for _ in 0..64 {
        let mut block = tone(amplitude, 4_096);
        effect.process(&mut block);
        peak = peak.max(block.peak());
    }
    peak
}

#[test]
fn a_loud_signal_never_passes_the_ceiling() {
    let peak = settle(-6.0, 2.0);
    assert!(peak <= math::db_to_gain(-6.0) + 1.0e-3, "{peak}");
}

#[test]
fn a_quiet_signal_is_brought_up() {
    let peak = settle(0.0, 0.02);
    assert!(peak > 0.25, "the maximiser did not raise the level: {peak}");
}

#[test]
fn a_signal_at_the_ceiling_is_left_alone() {
    let peak = settle(0.0, 0.5);
    assert!((peak - 1.0).abs() < 0.02, "{peak}");
}

#[test]
fn the_output_stays_finite_under_extremes() {
    let peak = settle(-30.0, 4.0);
    assert!(
        peak.is_finite() && peak <= math::db_to_gain(-30.0) + 1.0e-3,
        "{peak}"
    );
}

#[test]
fn silence_is_left_as_silence() {
    let peak = settle(0.0, 0.0);
    assert_eq!(peak, 0.0);
}

#[test]
fn a_zero_lookahead_does_not_delay_the_signal() {
    let params = ParamSet::new(specs());
    let mut effect = Maximizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    assert_eq!(effect.latency_frames(), 0);
    let mut impulse = AudioBuffer::new(2, 64);
    impulse.configure(2, 64).unwrap();
    impulse.channel_mut(0)[0] = 0.25;
    impulse.channel_mut(1)[0] = 0.25;
    effect.process(&mut impulse);
    assert!(impulse.channel(0)[0].abs() > 0.0);
}

#[test]
fn the_output_lags_by_the_lookahead() {
    let params = ParamSet::new(specs());
    params.set_f32("lookahead", 10.0).unwrap();
    let mut effect = Maximizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    assert_eq!(effect.latency_frames(), 480);
    let mut impulse = AudioBuffer::new(2, 1024);
    impulse.configure(2, 1024).unwrap();
    impulse.channel_mut(0)[0] = 0.25;
    impulse.channel_mut(1)[0] = 0.25;
    effect.process(&mut impulse);
    assert!(impulse.channel(0)[..480].iter().all(|value| *value == 0.0));
    assert!(impulse.channel(0)[480].abs() > 0.0);
}

#[test]
fn a_lookahead_keeps_the_ceiling() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -12.0).unwrap();
    params.set_f32("lookahead", 5.0).unwrap();
    let mut effect = Maximizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let ceiling = math::db_to_gain(-12.0);
    for _ in 0..32 {
        let mut block = tone(2.0, 4096);
        effect.process(&mut block);
        assert!(block.peak() <= ceiling + 1.0e-3, "{}", block.peak());
    }
}

#[test]
fn input_gain_scales_a_source_below_the_makeup_cap() {
    let measure = |gain: f32| {
        let params = ParamSet::new(specs());
        params.set_f32("input_gain", gain).unwrap();
        let mut effect = Maximizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut peak = 0.0f32;
        for _ in 0..64 {
            let mut block = tone(0.001, 4096);
            effect.process(&mut block);
            peak = peak.max(block.peak());
        }
        peak
    };
    let unity = measure(0.0);
    let lower = measure(-6.0);
    assert!(
        (lower / unity - 0.5).abs() < 0.05,
        "{lower} against {unity}"
    );
}

#[test]
fn output_gain_scales_the_limited_result() {
    let measure = |gain: f32| {
        let params = ParamSet::new(specs());
        params.set_f32("output_gain", gain).unwrap();
        let mut effect = Maximizer::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut peak = 0.0f32;
        for _ in 0..64 {
            let mut block = tone(0.5, 4096);
            effect.process(&mut block);
            peak = peak.max(block.peak());
        }
        peak
    };
    let unity = measure(0.0);
    let lower = measure(-6.0);
    assert!(
        (lower / unity - 0.5).abs() < 0.05,
        "{lower} against {unity}"
    );
}
