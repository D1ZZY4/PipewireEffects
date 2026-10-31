use pwe_core::AudioBuffer;

use crate::{BANDS, Loudness, SpectrumAnalyzer};

fn tone(amplitude: f32, frames: usize, phase: &mut f32) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    let step = std::f32::consts::TAU * 997.0 / 48_000.0;
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        let mut current = *phase;
        for sample in plan.iter_mut() {
            *sample = amplitude * current.sin();
            current += step;
        }
    }
    *phase = step.mul_add(frames as f32, *phase);
    buffer
}

#[test]
fn the_analyser_and_the_loudness_measurement_both_see_a_tone() {
    let mut analyser = SpectrumAnalyzer::new();
    analyser.prepare(48_000);
    let mut loudness = Loudness::new(48_000);
    let mut phase = 0.0f32;
    let mut bands = None;
    for _ in 0..60 {
        let buffer = tone(0.5, 4_800, &mut phase);
        loudness.process(&buffer);
        if let Some(measured) = analyser.push(&buffer) {
            bands = Some(measured);
        }
    }
    let bands = bands.expect("the analyser reports once its history is full");
    assert_eq!(bands.len(), BANDS);
    let loudest = bands.iter().copied().fold(f32::MIN, f32::max);
    assert!(loudest > -20.0, "the tone was not visible: {loudest}");
    let short_term = loudness.short_term().expect("three seconds were measured");
    assert!(short_term.is_finite() && short_term < 0.0, "{short_term}");
}
