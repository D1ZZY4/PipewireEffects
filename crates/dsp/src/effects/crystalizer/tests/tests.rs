use super::*;

fn tone(frequency: f32, amplitude: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = std::f32::consts::TAU * frequency * (offset + index) as f32 / 48_000.0;
            *sample = amplitude * phase.sin();
        }
    }
    buffer
}

fn impulse(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer.channel_mut(0)[0] = 1.0;
    buffer.channel_mut(1)[0] = 1.0;
    buffer
}

fn prepared(settings: &[(&str, f32)]) -> Crystalizer {
    let params = ParamSet::new(specs());
    for (id, value) in settings {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = Crystalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
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
fn there_are_thirteen_bands() {
    let specs = specs();
    assert_eq!(specs.len(), 4 + BANDS * 3);
    for ids in &BAND_IDS {
        for id in ids {
            assert!(specs.iter().any(|spec| spec.id == *id), "{id}");
        }
    }
    for id in ["adaptive_intensity", "oversampling", "transition_band"] {
        assert!(specs.iter().any(|spec| spec.id == id), "{id}");
    }
}

#[test]
fn transients_are_emphasised() {
    let mut effect = prepared(&[("band7_intensity", 32.0)]);
    let mut buffer = impulse(512);
    effect.process(&mut buffer);
    assert!(buffer.peak() > 1.0, "peak {}", buffer.peak());
}

#[test]
fn muting_a_band_removes_it() {
    let mut muted = prepared(&[("band1_mute", 1.0)]);
    let mut opened = prepared(&[]);
    let mut a = tone(48.0, 0.5, 8_192, 0);
    let mut b = tone(48.0, 0.5, 8_192, 0);
    for _ in 0..4 {
        muted.process(&mut a);
        opened.process(&mut b);
    }
    assert!(
        energy(&a) < energy(&b) * 0.2,
        "{} {}",
        energy(&a),
        energy(&b)
    );
    assert!(energy(&a).is_finite());
}

#[test]
fn a_bypassed_band_is_not_emphasised() {
    let mut effect = prepared(&[("band7_intensity", 32.0), ("band7_bypass", 1.0)]);
    let mut buffer = impulse(512);
    effect.process(&mut buffer);
    assert!(buffer.peak() <= 1.0 + 1.0e-6, "peak {}", buffer.peak());
}

#[test]
fn a_steady_tone_is_barely_changed() {
    let mut effect = prepared(&[("band5_intensity", 32.0)]);
    for block in 0..64 {
        let mut buffer = tone(700.0, 0.5, 1_024, block * 1_024);
        effect.process(&mut buffer);
    }
    let mut buffer = tone(700.0, 0.5, 1_024, 64 * 1_024);
    effect.process(&mut buffer);
    assert!(buffer.peak() < 0.75, "peak {}", buffer.peak());
}

#[test]
fn the_output_stays_finite_under_abuse() {
    let mut settings: Vec<(&str, f32)> = vec![("gain", 12.0)];
    for ids in &BAND_IDS {
        settings.push((ids[INTENSITY], 32.0));
    }
    let mut effect = prepared(&settings);
    for block in 0..64 {
        let mut buffer = tone(3_000.0, 1.0, 1_024, block * 1_024);
        buffer.channel_mut(0)[0] = 8.0;
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 4_096.0, "{sample}");
            }
        }
    }
}

#[test]
fn adaptive_intensity_strengthens_the_transient_emphasis() {
    let mut plain = prepared(&[("band7_intensity", 32.0)]);
    let mut adaptive = prepared(&[("band7_intensity", 32.0), ("adaptive_intensity", 1.0)]);
    let mut first = impulse(512);
    let mut second = impulse(512);
    plain.process(&mut first);
    adaptive.process(&mut second);
    assert!(
        second.peak() > first.peak(),
        "{} against {}",
        second.peak(),
        first.peak()
    );
}

#[test]
fn oversampling_preserves_a_tone_level() {
    let mut effect = prepared(&[("oversampling", 1.0)]);
    for block in 0..16 {
        let mut buffer = tone(700.0, 0.5, 1_024, block * 1_024);
        effect.process(&mut buffer);
    }
    let mut buffer = tone(700.0, 0.5, 1_024, 16 * 1_024);
    effect.process(&mut buffer);
    assert!((buffer.peak() - 0.5).abs() < 0.02, "peak {}", buffer.peak());
}

#[test]
fn oversampling_selects_the_double_rate_path() {
    let effect = prepared(&[("oversampling", 1.0)]);
    assert_eq!(effect.factor, 2);
    assert_eq!(effect.signature.1, effect.stages);
}

#[test]
fn oversampling_changes_the_transient_emphasis() {
    let mut plain = prepared(&[("band7_intensity", 32.0)]);
    let mut doubled = prepared(&[("band7_intensity", 32.0), ("oversampling", 1.0)]);
    let mut first = impulse(512);
    let mut second = impulse(512);
    plain.process(&mut first);
    doubled.process(&mut second);
    assert!(
        (first.peak() - second.peak()).abs() > 1.0e-4,
        "{} against {}",
        first.peak(),
        second.peak()
    );
}

#[test]
fn oversampling_stays_finite_under_abuse() {
    let mut settings: Vec<(&str, f32)> = vec![("oversampling", 1.0), ("gain", 12.0)];
    for ids in &BAND_IDS {
        settings.push((ids[INTENSITY], 32.0));
    }
    let mut effect = prepared(&settings);
    for block in 0..32 {
        let mut buffer = tone(3_000.0, 1.0, 1_024, block * 1_024);
        buffer.channel_mut(0)[0] = 8.0;
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 4_096.0, "{sample}");
            }
        }
    }
}

#[test]
fn a_narrower_transition_band_cascades_more_filter_stages() {
    assert_eq!(stage_count(1_000.0), 1);
    assert_eq!(stage_count(120.0), 1);
    assert_eq!(stage_count(60.0), 2);
    assert_eq!(stage_count(40.0), 3);
    assert_eq!(stage_count(30.0), 4);
    assert_eq!(stage_count(10.0), 4);
}

#[test]
fn the_transition_band_changes_the_emphasis() {
    let mut wide = prepared(&[("band5_intensity", 32.0), ("transition_band", 1_000.0)]);
    let mut narrow = prepared(&[("band5_intensity", 32.0), ("transition_band", 30.0)]);
    let mut first = impulse(512);
    let mut second = impulse(512);
    wide.process(&mut first);
    narrow.process(&mut second);
    assert!(
        (first.peak() - second.peak()).abs() > 1.0e-4,
        "{} against {}",
        first.peak(),
        second.peak()
    );
}
