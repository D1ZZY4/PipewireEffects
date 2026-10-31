use super::*;

fn settings(pairs: &[(&str, f32)]) -> ParamSet {
    let params = ParamSet::new(specs());
    for (key, value) in pairs {
        params.set_f32(key, *value).unwrap();
    }
    params
}

fn tone(frequency: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = 0.4 * (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn settled(pairs: &[(&str, f32)], frequency: f32) -> AudioBuffer {
    let mut effect = Exciter::new(settings(pairs));
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(frequency, 4_096);
    for _ in 0..8 {
        buffer = tone(frequency, 4_096);
        effect.process(&mut buffer);
    }
    buffer
}

#[test]
fn a_signal_below_the_scope_is_nearly_unchanged() {
    let processed = settled(&[("amount", 0.0), ("harmonics", 8.5)], 200.0);
    assert!(processed.peak() < 0.45, "{}", processed.peak());
}

#[test]
fn the_scope_band_produces_more_energy() {
    let dry = settled(&[("amount", -24.0)], 6_000.0).rms();
    let excited = settled(&[("amount", 0.0)], 6_000.0).rms();
    assert!(excited > dry * 1.05, "{excited} against {dry}");
}

#[test]
fn monitoring_the_harmonics_removes_the_fundamental() {
    let processed = settled(&[("listen", 1.0)], 6_000.0);
    assert!(processed.peak() > 0.0);
    assert!(processed.peak() < 1.0, "{}", processed.peak());
}

#[test]
fn the_ceiling_band_can_be_bounded() {
    let processed = settled(&[("ceil_active", 1.0), ("ceil", 10_000.0)], 18_000.0);
    assert!(processed.peak() < 0.6, "{}", processed.peak());
}

#[test]
fn extreme_settings_stay_finite_and_bounded() {
    let processed = settled(&[("amount", 24.0), ("harmonics", 10.0)], 6_000.0);
    assert!(processed.peak().is_finite() && processed.peak() < 8.0);
}
