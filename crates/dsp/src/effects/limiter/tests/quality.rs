use super::*;

fn shifted_tone(amplitude: f32, frequency: f32, phase: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let angle = std::f32::consts::TAU * frequency * index as f32 / 48_000.0 + phase;
            *sample = amplitude * angle.sin();
        }
    }
    buffer
}

#[test]
fn four_times_oversampling_passes_a_quiet_tone_at_unity() {
    let params = ParamSet::new(specs());
    params.set_choice("oversampling", 2).unwrap();
    let mut effect = prepared(params);
    let mut buffer = shifted_tone(0.05, 1_000.0, 0.0, 4096);
    for _ in 0..16 {
        buffer = shifted_tone(0.05, 1_000.0, 0.0, 4096);
        effect.process(&mut buffer);
    }
    assert!(
        (buffer.peak() - 0.05).abs() < 0.001,
        "the resampler changed the level: {}",
        buffer.peak()
    );
}

#[test]
fn oversampling_detects_an_inter_sample_peak() {
    let measure = |factor: u16| {
        let params = ParamSet::new(specs());
        params.set_f32("limit", -2.0).unwrap();
        params.set_f32("lookahead", 20.0).unwrap();
        params.set_choice("oversampling", factor).unwrap();
        let mut effect = prepared(params);
        let mut buffer = shifted_tone(1.0, 12_000.0, std::f32::consts::FRAC_PI_4, 4096);
        for _ in 0..8 {
            buffer = shifted_tone(1.0, 12_000.0, std::f32::consts::FRAC_PI_4, 4096);
            effect.process(&mut buffer);
        }
        buffer.peak()
    };
    let sampled = measure(0);
    let oversampled = measure(2);
    assert!(
        sampled > 0.68,
        "the sampled path limited material that was under the ceiling: {sampled}"
    );
    assert!(
        oversampled < 0.62 && oversampled > 0.45,
        "true peak detection did not engage: {oversampled}"
    );
}

#[test]
fn oversampling_adds_the_resampler_latency() {
    let params = ParamSet::new(specs());
    params.set_f32("lookahead", 10.0).unwrap();
    params.set_choice("oversampling", 2).unwrap();
    let effect = prepared(params);
    assert_eq!(effect.latency_frames(), 480 + 16);
}
