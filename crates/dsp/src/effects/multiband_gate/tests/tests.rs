use super::*;

fn tone(frequency: f32, amplitude: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude
                * (std::f32::consts::TAU * frequency * (offset + index) as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn peak_after(effect: &mut MultibandGate, amplitude: f32, blocks: usize) -> f32 {
    let frames = 1024;
    let mut peak = 0.0f32;
    for block in 0..blocks * 2 {
        let mut buffer = tone(1_000.0, amplitude, frames, block * frames);
        effect.process(&mut buffer);
        if block + blocks >= blocks * 2 {
            peak = peak.max(buffer.peak());
        }
    }
    peak
}

#[test]
fn a_loud_signal_stays_open() {
    let params = ParamSet::new(specs());
    for ids in BAND_IDS {
        params.set_f32(ids[THRESHOLD], -40.0).unwrap();
    }
    let mut effect = MultibandGate::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let peak = peak_after(&mut effect, 0.5, 32);
    assert!((peak - 0.5).abs() < 0.05, "{peak}");
}

#[test]
fn a_quiet_signal_is_closed_by_the_range() {
    let params = ParamSet::new(specs());
    for ids in BAND_IDS {
        params.set_f32(ids[THRESHOLD], -20.0).unwrap();
        params.set_f32(ids[RANGE], 60.0).unwrap();
        params.set_f32(ids[HOLD], 0.0).unwrap();
        params.set_f32(ids[RELEASE], 10.0).unwrap();
    }
    let mut effect = MultibandGate::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let peak = peak_after(&mut effect, 0.001, 64);
    let expected = 0.001 * math::db_to_gain(-60.0);
    assert!((peak - expected).abs() < 1.0e-5, "{peak} {expected}");
}

#[test]
fn a_disabled_band_is_not_gated() {
    let params = ParamSet::new(specs());
    for ids in BAND_IDS {
        params.set_f32(ids[ENABLED], 0.0).unwrap();
        params.set_f32(ids[THRESHOLD], -20.0).unwrap();
        params.set_f32(ids[RANGE], 60.0).unwrap();
    }
    let mut effect = MultibandGate::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let peak = peak_after(&mut effect, 0.001, 64);
    assert!((peak - 0.001).abs() < 1.0e-4, "{peak}");
}
