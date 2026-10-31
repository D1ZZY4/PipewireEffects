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
