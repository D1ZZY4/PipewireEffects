use super::*;

fn settings(pairs: &[(&str, f32)]) -> ParamSet {
    let params = ParamSet::new(specs());
    for (key, value) in pairs {
        params.set_f32(key, *value).unwrap();
    }
    params
}

fn tone(left: f32, right: f32, frequency: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for index in 0..frames {
        let value = (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
        buffer.channel_mut(0)[index] = value * left;
        buffer.channel_mut(1)[index] = value * right;
    }
    buffer
}

fn settled(pairs: &[(&str, f32)], left: f32, right: f32, frequency: f32) -> AudioBuffer {
    let mut effect = MidSideEqualizer::new(settings(pairs));
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    let mut last = tone(left, right, frequency, 2_048);
    for _ in 0..32 {
        last = tone(left, right, frequency, 2_048);
        effect.process(&mut last);
        peak = peak.max(last.peak());
    }
    assert!(peak.is_finite());
    last
}

#[test]
fn a_flat_curve_is_transparent() {
    let processed = settled(&[], 0.5, -0.5, 1_000.0);
    let reference = tone(0.5, -0.5, 1_000.0, 2_048);
    for channel in 0..2 {
        for (processed, original) in processed
            .channel(channel)
            .iter()
            .zip(reference.channel(channel))
        {
            assert!(
                (processed - original).abs() < 1.0e-3,
                "{processed} {original}"
            );
        }
    }
}

#[test]
fn a_mid_band_boost_changes_a_mono_signal() {
    let processed = settled(&[("mid_band6_gain", 8.0)], 0.4, 0.4, 1_000.0);
    assert!(processed.peak() > 0.4 + 0.2, "{}", processed.peak());
}

#[test]
fn a_side_band_boost_leaves_a_mono_signal_alone() {
    let processed = settled(&[("side_band6_gain", 12.0)], 0.4, 0.4, 1_000.0);
    assert!(
        (processed.peak() - 0.4).abs() < 0.05,
        "{}",
        processed.peak()
    );
}

#[test]
fn the_side_gain_widens_and_narrows_the_image() {
    let wide = settled(&[("side_gain", 6.0)], 0.3, -0.3, 500.0);
    let narrow = settled(&[("side_gain", -6.0)], 0.3, -0.3, 500.0);
    assert!(wide.peak() > narrow.peak() * 1.5);
}

#[test]
fn the_output_gain_is_applied() {
    let processed = settled(&[("output_gain", -6.0)], 0.4, -0.4, 800.0);
    assert!((processed.peak() - 0.4 * math::db_to_gain(-6.0)).abs() < 0.02);
}
