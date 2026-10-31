use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 250.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn prepared(params: ParamSet) -> Gate {
    let mut effect = Gate::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

#[test]
fn loud_signal_stays_open() {
    let mut effect = prepared(ParamSet::new(specs()));
    let mut buffer = tone(0.5, 4096);
    effect.process(&mut buffer);
    assert!((buffer.peak() - 0.5).abs() < 0.01, "{}", buffer.peak());
}

#[test]
fn quiet_signal_is_attenuated_by_the_range() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -20.0).unwrap();
    params.set_f32("range", 60.0).unwrap();
    params.set_f32("curve", 0.0).unwrap();
    params.set_f32("hold", 0.0).unwrap();
    params.set_f32("release", 5.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = tone(0.001, 4096);
    for _ in 0..256 {
        buffer = tone(0.001, 4096);
        effect.process(&mut buffer);
    }
    let expected = 0.001 * math::db_to_gain(-60.0);
    assert!(
        (buffer.peak() - expected).abs() < 1.0e-5,
        "{} {}",
        buffer.peak(),
        expected
    );
}

#[test]
fn hold_delays_the_close() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -20.0).unwrap();
    params.set_f32("hold", 500.0).unwrap();
    let mut effect = prepared(params);
    let mut loud = tone(0.5, 1024);
    effect.process(&mut loud);
    let mut quiet = tone(0.0001, 512);
    effect.process(&mut quiet);
    assert!(
        quiet.peak() > 0.00005,
        "gate closed too early: {}",
        quiet.peak()
    );
}

#[test]
fn the_curve_reduces_in_steps_instead_of_switching() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -20.0).unwrap();
    params.set_f32("range", 60.0).unwrap();
    params.set_f32("curve", 20.0).unwrap();
    params.set_f32("hold", 0.0).unwrap();
    params.set_f32("attack", 50.0).unwrap();
    params.set_f32("release", 50.0).unwrap();
    let mut effect = prepared(params);
    let mut half = tone(0.05, 8192);
    effect.process(&mut half);
    let half = half.peak();
    for _ in 0..64 {
        let mut settle = tone(0.0005, 8192);
        effect.process(&mut settle);
    }
    let fully_closed = 0.0005 * math::db_to_gain(-60.0);
    assert!(
        half > fully_closed * 1.5,
        "the curve did not soften the transition: {half} against {fully_closed}"
    );
}

#[test]
fn hysteresis_keeps_the_gate_open_below_the_threshold() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -20.0).unwrap();
    params.set_f32("hysteresis", 12.0).unwrap();
    params.set_f32("curve", 0.0).unwrap();
    params.set_f32("hold", 0.0).unwrap();
    params.set_f32("release", 5.0).unwrap();
    let mut effect = prepared(params);
    let mut loud = tone(0.5, 8192);
    effect.process(&mut loud);
    let mut below = tone(0.05, 8192);
    for _ in 0..16 {
        effect.process(&mut below);
        below = tone(0.05, 8192);
    }
    assert!(
        (below.peak() - 0.05).abs() < 0.001,
        "the gate closed inside the hysteresis window: {}",
        below.peak()
    );
    let mut very_quiet = tone(0.002, 8192);
    for _ in 0..256 {
        very_quiet = tone(0.002, 8192);
        effect.process(&mut very_quiet);
    }
    let expected = 0.002 * math::db_to_gain(-40.0);
    assert!(
        (very_quiet.peak() - expected).abs() < 1.0e-5,
        "the gate stayed open below the close point: {} against {expected}",
        very_quiet.peak()
    );
}
