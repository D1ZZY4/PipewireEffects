use super::*;

fn tone(frequency: f32, amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample =
                amplitude * (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

#[test]
fn sibilance_is_reduced() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -30.0).unwrap();
    params.set_f32("ratio", 8.0).unwrap();
    let mut effect = DeEsser::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    for _ in 0..256 {
        let mut buffer = tone(6_000.0, 0.7, 1024);
        effect.process(&mut buffer);
    }
    let mut buffer = tone(6_000.0, 0.7, 1024);
    effect.process(&mut buffer);
    assert!(buffer.peak() < 0.55, "peak {}", buffer.peak());
}

#[test]
fn low_frequencies_are_untouched() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -30.0).unwrap();
    let mut effect = DeEsser::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    for _ in 0..256 {
        let mut buffer = tone(300.0, 0.5, 1024);
        effect.process(&mut buffer);
    }
    let mut buffer = tone(300.0, 0.5, 1024);
    effect.process(&mut buffer);
    assert!((buffer.peak() - 0.5).abs() < 0.02, "peak {}", buffer.peak());
}

#[test]
fn output_is_always_finite() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -40.0).unwrap();
    params.set_f32("ratio", 10.0).unwrap();
    params.set_f32("frequency", 2_000.0).unwrap();
    let mut effect = DeEsser::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    for _ in 0..200 {
        let mut buffer = tone(2_000.0, 1.0, 1024);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 4.0, "{sample}");
            }
        }
    }
}

#[test]
fn changing_the_detector_frequency_takes_effect_while_processing() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -30.0).unwrap();
    params.set_f32("ratio", 8.0).unwrap();
    let mut effect = DeEsser::new(params.clone());
    effect.prepare(AudioFormat::new(48_000, 2));
    for _ in 0..256 {
        let mut buffer = tone(6_000.0, 0.7, 1024);
        effect.process(&mut buffer);
    }
    let mut reduced = tone(6_000.0, 0.7, 1024);
    effect.process(&mut reduced);
    assert!(reduced.peak() < 0.55, "peak {}", reduced.peak());

    params.set_f32("frequency", 2_000.0).unwrap();
    for _ in 0..256 {
        let mut buffer = tone(6_000.0, 0.7, 1024);
        effect.process(&mut buffer);
    }
    let mut restored = tone(6_000.0, 0.7, 1024);
    effect.process(&mut restored);
    assert!(
        restored.peak() > 0.65,
        "moving the detector away left the tone reduced: {}",
        restored.peak()
    );
}
