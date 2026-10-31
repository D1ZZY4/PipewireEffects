use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    tone_from(amplitude, frames, 0)
}

fn tone_from(amplitude: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = std::f32::consts::TAU * 250.0 * (offset + index) as f32 / 48_000.0;
            *sample = amplitude * phase.sin();
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

#[test]
fn an_explicit_close_threshold_holds_the_gate_open() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -20.0).unwrap();
    params.set_f32("hysteresis_threshold", -45.0).unwrap();
    params.set_f32("curve", 0.0).unwrap();
    params.set_f32("hold", 0.0).unwrap();
    params.set_f32("release", 5.0).unwrap();
    let mut effect = prepared(params);
    let mut loud = tone(0.5, 8192);
    effect.process(&mut loud);
    let mut below = tone(0.02, 8192);
    for _ in 0..16 {
        effect.process(&mut below);
        below = tone(0.02, 8192);
    }
    assert!(
        (below.peak() - 0.02).abs() < 0.001,
        "the gate closed above its explicit close threshold: {}",
        below.peak()
    );
    let mut quiet = tone(0.0005, 8192);
    for _ in 0..256 {
        quiet = tone(0.0005, 8192);
        effect.process(&mut quiet);
    }
    let expected = 0.0005 * math::db_to_gain(-40.0);
    assert!(
        (quiet.peak() - expected).abs() < 1.0e-6,
        "{} against {expected}",
        quiet.peak()
    );
}

#[test]
fn the_explicit_close_threshold_wins_over_the_width() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -20.0).unwrap();
    params.set_f32("hysteresis", 30.0).unwrap();
    params.set_f32("hysteresis_threshold", -25.0).unwrap();
    params.set_f32("curve", 0.0).unwrap();
    params.set_f32("hold", 0.0).unwrap();
    params.set_f32("release", 5.0).unwrap();
    let mut effect = prepared(params);
    let mut loud = tone(0.5, 8192);
    effect.process(&mut loud);
    let mut below = tone(0.02, 8192);
    for _ in 0..64 {
        below = tone(0.02, 8192);
        effect.process(&mut below);
    }
    assert!(
        below.peak() < 1.0e-3,
        "the width overrode the explicit close threshold: {}",
        below.peak()
    );
}

#[test]
fn the_close_zone_softens_the_close() {
    let measure = |zone: f32| {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -20.0).unwrap();
        params.set_f32("hysteresis_threshold", -30.0).unwrap();
        params.set_f32("hysteresis_zone", zone).unwrap();
        params.set_f32("range", 60.0).unwrap();
        params.set_f32("curve", 0.0).unwrap();
        params.set_f32("hold", 0.0).unwrap();
        params.set_f32("attack", 50.0).unwrap();
        params.set_f32("release", 50.0).unwrap();
        let mut effect = prepared(params);
        let mut buffer = tone(0.008, 8192);
        for _ in 0..256 {
            buffer = tone(0.008, 8192);
            effect.process(&mut buffer);
        }
        buffer.peak()
    };
    let hard = measure(0.0);
    let soft = measure(20.0);
    assert!(
        soft > hard * 5.0,
        "the close zone did not soften the transition: {soft} against {hard}"
    );
}

#[test]
fn the_sidechain_high_pass_keeps_rumble_out_of_the_detector() {
    let base = ParamSet::new(specs());
    base.set_f32("threshold", -30.0).unwrap();
    base.set_f32("range", 60.0).unwrap();
    base.set_f32("curve", 0.0).unwrap();
    base.set_f32("hold", 0.0).unwrap();
    base.set_f32("attack", 5.0).unwrap();
    base.set_f32("release", 5.0).unwrap();
    base.set_f32("hysteresis", 0.0).unwrap();
    let filtered_params = ParamSet::new(specs());
    filtered_params.set_f32("threshold", -30.0).unwrap();
    filtered_params.set_f32("range", 60.0).unwrap();
    filtered_params.set_f32("curve", 0.0).unwrap();
    filtered_params.set_f32("hold", 0.0).unwrap();
    filtered_params.set_f32("attack", 5.0).unwrap();
    filtered_params.set_f32("release", 5.0).unwrap();
    filtered_params.set_f32("hysteresis", 0.0).unwrap();
    filtered_params.set_choice("hpf_mode", 3).unwrap();
    filtered_params.set_f32("hpf_frequency", 2_000.0).unwrap();
    let mut open = prepared(base);
    let mut filtered = prepared(filtered_params);

    let frames = 8_192;
    let mut open_buffer = tone_from(0.5, frames, 0);
    for block in 0..16 {
        open_buffer = tone_from(0.5, frames, block * frames);
        open.process(&mut open_buffer);
    }
    let mut filtered_buffer = tone_from(0.5, frames, 0);
    for block in 0..16 {
        filtered_buffer = tone_from(0.5, frames, block * frames);
        filtered.process(&mut filtered_buffer);
    }
    assert!(open_buffer.peak() > 0.4, "{}", open_buffer.peak());
    let closed = 0.5 * math::db_to_gain(-60.0);
    assert!(
        filtered_buffer.peak() < closed * 4.0,
        "the rumble still opened the gate: {} against {closed}",
        filtered_buffer.peak()
    );
}
