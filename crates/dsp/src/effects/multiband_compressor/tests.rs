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

fn settled(effect: &mut MultibandCompressor, frequency: f32, amplitude: f32) -> f32 {
    let frames = 1024;
    let blocks = 64;
    let mut peak = 0.0f32;
    for block in 0..blocks {
        let mut buffer = tone(frequency, amplitude, frames, block * frames);
        effect.process(&mut buffer);
        if block + 16 >= blocks {
            peak = peak.max(buffer.peak());
        }
    }
    peak
}

#[test]
fn a_unity_ratio_is_transparent() {
    let params = ParamSet::new(specs());
    for ids in BAND_IDS {
        params.set_f32(ids[RATIO], 1.0).unwrap();
    }
    let mut effect = MultibandCompressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let peak = settled(&mut effect, 1_000.0, 0.5);
    assert!((peak - 0.5).abs() < 0.05, "{peak}");
}

#[test]
fn loud_material_is_reduced() {
    let params = ParamSet::new(specs());
    params.set_f32("bands", 2.0).unwrap();
    for ids in BAND_IDS.iter().take(2) {
        params.set_f32(ids[THRESHOLD], -20.0).unwrap();
        params.set_f32(ids[RATIO], 8.0).unwrap();
    }
    let mut effect = MultibandCompressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let peak = settled(&mut effect, 1_000.0, 0.5);
    assert!(peak < 0.35, "the compressor did not act: {peak}");
}

#[test]
fn makeup_lifts_the_compressed_band() {
    let measure = |makeup: f32| {
        let params = ParamSet::new(specs());
        params.set_f32("bands", 2.0).unwrap();
        for ids in BAND_IDS.iter().take(2) {
            params.set_f32(ids[THRESHOLD], -20.0).unwrap();
            params.set_f32(ids[RATIO], 8.0).unwrap();
            params.set_f32(ids[MAKEUP], makeup).unwrap();
        }
        let mut effect = MultibandCompressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        settled(&mut effect, 1_000.0, 0.5)
    };
    let without = measure(0.0);
    let with = measure(6.0);
    assert!(with > without * 1.8, "{with} against {without}");
}

#[test]
fn a_disabled_band_passes_its_range_through() {
    let params = ParamSet::new(specs());
    params.set_f32("bands", 2.0).unwrap();
    for ids in BAND_IDS.iter().take(2) {
        params.set_f32(ids[ENABLED], 0.0).unwrap();
    }
    let mut effect = MultibandCompressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let peak = settled(&mut effect, 200.0, 0.5);
    assert!((peak - 0.5).abs() < 0.05, "{peak}");
}
