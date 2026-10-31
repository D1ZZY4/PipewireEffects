use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 400.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn settled(settings: &[(&str, f32)], amplitude: f32) -> f32 {
    settled_in_mode(0, settings, amplitude)
}

fn settled_in_mode(mode: u16, settings: &[(&str, f32)], amplitude: f32) -> f32 {
    let params = ParamSet::new(specs());
    params.set_f32("knee", 0.0).unwrap();
    params.set_f32("attack", 0.5).unwrap();
    params.set_f32("release", 0.5).unwrap();
    params.set_choice("mode", mode).unwrap();
    for (key, value) in settings {
        params.set_f32(key, *value).unwrap();
    }
    let mut effect = Expander::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(amplitude, 1024);
    for _ in 0..256 {
        buffer = tone(amplitude, 1024);
        effect.process(&mut buffer);
    }
    buffer.peak()
}

#[test]
fn signal_above_the_threshold_is_untouched() {
    let peak = settled(&[], 0.5);
    assert!((peak - 0.5).abs() < 0.01, "{peak}");
}

#[test]
fn signal_below_the_threshold_is_expanded_downwards() {
    let peak = settled(&[], 0.01);
    assert!(peak < 1.0e-5, "the quiet signal was not expanded: {peak}");
}

#[test]
fn a_higher_ratio_expands_harder() {
    let gentle = settled(&[("ratio", 2.0)], 0.01);
    let aggressive = settled(&[("ratio", 20.0)], 0.01);
    assert!(aggressive < gentle, "{aggressive} !< {gentle}");
}

#[test]
fn makeup_is_applied_to_the_expanded_signal() {
    let plain = settled(&[("threshold", -12.0), ("ratio", 2.0)], 0.05);
    let lifted = settled(
        &[("threshold", -12.0), ("ratio", 2.0), ("makeup", 12.0)],
        0.05,
    );
    assert!(
        (math::gain_to_db(lifted) - math::gain_to_db(plain) - 12.0).abs() < 0.5,
        "{} {}",
        math::gain_to_db(plain),
        math::gain_to_db(lifted)
    );
}

#[test]
fn a_zero_mix_keeps_the_dry_signal() {
    let peak = settled(&[("mix", 0.0)], 0.01);
    assert!((peak - 0.01).abs() < 1.0e-4, "{peak}");
}

#[test]
fn upward_mode_boosts_material_above_the_threshold() {
    let settings = [("threshold", -12.0), ("ratio", 4.0)];
    let downward = settled_in_mode(0, &settings, 0.5);
    let upward = settled_in_mode(1, &settings, 0.5);
    assert!((downward - 0.5).abs() < 0.01, "{downward}");
    assert!(
        upward > 1.0,
        "the upward mode did not raise the level: {upward}"
    );
}

#[test]
fn upward_mode_leaves_material_below_the_threshold_alone() {
    let settings = [("threshold", -12.0), ("ratio", 4.0)];
    let peak = settled_in_mode(1, &settings, 0.01);
    assert!((peak - 0.01).abs() < 1.0e-4, "{peak}");
}

#[test]
fn upward_boost_is_bounded() {
    let settings = [("threshold", -60.0), ("ratio", 100.0)];
    let peak = settled_in_mode(1, &settings, 0.9);
    assert!(peak.is_finite(), "{peak}");
    assert!(
        peak < 0.9 * 20.0,
        "the upward boost was not bounded: {peak}"
    );
}

#[test]
fn upward_mode_stays_finite_at_the_extremes() {
    let settings = [
        ("threshold", -60.0),
        ("ratio", 100.0),
        ("makeup", 60.0),
        ("mix", 100.0),
    ];
    let peak = settled_in_mode(1, &settings, 1.0);
    assert!(peak.is_finite(), "{peak}");
    assert!(peak.abs() < 1.0e6, "{peak}");
}

#[test]
fn extreme_settings_stay_finite() {
    let peak = settled(
        &[("ratio", 100.0), ("threshold", -60.0), ("makeup", 60.0)],
        1.0,
    );
    assert!(peak.is_finite(), "{peak}");
    assert!(peak.abs() < 1.0e6, "{peak}");
}
