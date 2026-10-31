use super::*;

fn tone(frequency: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = 0.5 * (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

#[test]
fn a_bass_tone_gains_harmonic_content() {
    let params = ParamSet::new(specs());
    params.set_f32("amount", 100.0).unwrap();
    let mut effect = BassEnhancer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(60.0, 4096);
    for _ in 0..64 {
        buffer = tone(60.0, 4096);
        effect.process(&mut buffer);
    }
    assert!(buffer.peak() > 0.5, "peak {}", buffer.peak());
}

#[test]
fn with_zero_amount_the_signal_is_unchanged() {
    let params = ParamSet::new(specs());
    params.set_f32("amount", 0.0).unwrap();
    let mut effect = BassEnhancer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(60.0, 2048);
    effect.process(&mut buffer);
    assert!((buffer.peak() - 0.5).abs() < 0.01, "peak {}", buffer.peak());
}

#[test]
fn output_stays_finite() {
    let params = ParamSet::new(specs());
    params.set_f32("amount", 100.0).unwrap();
    params.set_f32("gain", 12.0).unwrap();
    let mut effect = BassEnhancer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    for _ in 0..64 {
        let mut buffer = tone(40.0, 4096);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 8.0, "{sample}");
            }
        }
    }
}

fn rendered(settings: &[(&str, f32)], frequency: f32) -> AudioBuffer {
    let params = ParamSet::new(specs());
    for (id, value) in settings {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = BassEnhancer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    for _ in 0..64 {
        let mut buffer = tone(frequency, 4096);
        effect.process(&mut buffer);
    }
    let mut buffer = tone(frequency, 4096);
    effect.process(&mut buffer);
    buffer
}

fn energy(buffer: &AudioBuffer) -> f64 {
    let mut sum = 0.0;
    for channel in 0..2 {
        for sample in buffer.channel(channel) {
            sum += f64::from(*sample) * f64::from(*sample);
        }
    }
    sum
}

#[test]
fn the_new_controls_are_published() {
    let specs = specs();
    for id in ["harmonics", "blend", "floor", "floor_active", "listen"] {
        assert!(specs.iter().any(|spec| spec.id == id), "{id}");
    }
}

#[test]
fn a_higher_harmonics_control_adds_more_energy() {
    let gentle = energy(&rendered(&[("amount", 100.0), ("harmonics", 1.0)], 80.0));
    let driven = energy(&rendered(&[("amount", 100.0), ("harmonics", 9.0)], 80.0));
    assert!(driven > gentle * 1.1, "{driven} against {gentle}");
}

#[test]
fn the_floor_filter_reduces_the_generated_bass() {
    let open = energy(&rendered(&[("amount", 100.0), ("floor_active", 0.0)], 60.0));
    let floored = energy(&rendered(
        &[("amount", 100.0), ("floor_active", 1.0), ("floor", 100.0)],
        60.0,
    ));
    assert!(floored < open, "{floored} against {open}");
}

#[test]
fn listen_removes_the_dry_path() {
    let silent = energy(&rendered(&[("amount", 0.0), ("listen", 1.0)], 60.0));
    let dry = energy(&rendered(&[("amount", 0.0), ("listen", 0.0)], 60.0));
    assert!(dry > 0.0);
    assert!(
        silent < dry * 1.0e-4,
        "listen kept the dry signal: {silent} against {dry}"
    );
}

#[test]
fn blend_changes_the_generated_curve() {
    let odd = rendered(&[("amount", 100.0), ("blend", -10.0)], 80.0);
    let even = rendered(&[("amount", 100.0), ("blend", 10.0)], 80.0);
    let mut difference = 0.0f64;
    for (left, right) in odd.channel(0).iter().zip(even.channel(0)) {
        difference += f64::from((left - right).abs());
    }
    assert!(
        difference > 1.0,
        "the blend control did nothing: {difference}"
    );
}
