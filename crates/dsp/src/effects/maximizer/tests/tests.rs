use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 500.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn settle(threshold: f32, amplitude: f32) -> f32 {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", threshold).unwrap();
    let mut effect = Maximizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    for _ in 0..64 {
        let mut block = tone(amplitude, 4_096);
        effect.process(&mut block);
        peak = peak.max(block.peak());
    }
    peak
}

#[test]
fn a_loud_signal_never_passes_the_ceiling() {
    let peak = settle(-6.0, 2.0);
    assert!(peak <= math::db_to_gain(-6.0) + 1.0e-3, "{peak}");
}

#[test]
fn a_quiet_signal_is_brought_up() {
    let peak = settle(0.0, 0.02);
    assert!(peak > 0.25, "the maximiser did not raise the level: {peak}");
}

#[test]
fn a_signal_at_the_ceiling_is_left_alone() {
    let peak = settle(0.0, 0.5);
    assert!((peak - 1.0).abs() < 0.02, "{peak}");
}

#[test]
fn the_output_stays_finite_under_extremes() {
    let peak = settle(-30.0, 4.0);
    assert!(
        peak.is_finite() && peak <= math::db_to_gain(-30.0) + 1.0e-3,
        "{peak}"
    );
}

#[test]
fn silence_is_left_as_silence() {
    let peak = settle(0.0, 0.0);
    assert_eq!(peak, 0.0);
}
