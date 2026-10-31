use super::*;

fn tone(frequency: f32, frames: usize, left: f32, right: f32) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for index in 0..frames {
        let value = (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
        buffer.channel_mut(0)[index] = value * left;
        buffer.channel_mut(1)[index] = value * right;
    }
    buffer
}

fn effect(cutoff: f32, feed: f32) -> Crossfeed {
    let params = ParamSet::new(specs());
    params.set_f32("cutoff", cutoff).unwrap();
    params.set_f32("feed", feed).unwrap();
    let mut effect = Crossfeed::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

fn leak(cutoff: f32, feed: f32, frequency: f32) -> f32 {
    let mut effect = effect(cutoff, feed);
    let mut buffer = tone(frequency, 9_600, 0.0, 0.5);
    for _ in 0..4 {
        buffer = tone(frequency, 9_600, 0.0, 0.5);
        effect.process(&mut buffer);
    }
    buffer
        .channel(0)
        .iter()
        .fold(0.0f32, |peak, value| peak.max(value.abs()))
}

#[test]
fn a_mono_signal_passes_through_unchanged() {
    let mut effect = effect(700.0, 4.5);
    let mut buffer = tone(200.0, 4096, 0.5, 0.5);
    let source: Vec<f32> = buffer.channel(0).to_vec();
    effect.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(source) {
        assert!(
            (processed - original).abs() < 1.0e-6,
            "{processed} {original}"
        );
    }
}

#[test]
fn low_frequencies_cross_to_the_other_channel() {
    let measured = leak(700.0, 9.5, 100.0);
    assert!(
        math::gain_to_db(measured) > -6.0,
        "{} dB",
        math::gain_to_db(measured)
    );
}

#[test]
fn high_frequencies_stay_where_they_are() {
    let low = leak(700.0, 9.5, 100.0);
    let high = leak(700.0, 9.5, 10_000.0);
    assert!(high < low * 0.5, "{high} against {low}");
}

#[test]
fn a_lower_cutoff_crosses_less() {
    let wide = leak(2_000.0, 9.5, 1_000.0);
    let narrow = leak(300.0, 9.5, 1_000.0);
    assert!(wide > narrow, "{wide} !> {narrow}");
}

#[test]
fn a_single_channel_leaves_the_processed_side_finite() {
    let mut effect = effect(700.0, 15.0);
    let mut buffer = tone(80.0, 4096, 0.9, -0.9);
    effect.process(&mut buffer);
    for channel in 0..2 {
        for sample in buffer.channel(channel) {
            assert!(sample.is_finite(), "{sample}");
            assert!(sample.abs() < 10.0, "{sample}");
        }
    }
}

fn gain_effect(input_gain: f32, output_gain: f32) -> Crossfeed {
    let params = ParamSet::new(specs());
    params.set_f32("input_gain", input_gain).unwrap();
    params.set_f32("output_gain", output_gain).unwrap();
    let mut effect = Crossfeed::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

#[test]
fn the_input_gain_scales_the_input() {
    let mut effect = gain_effect(-6.0, 0.0);
    let mut buffer = tone(200.0, 4096, 0.5, 0.5);
    effect.process(&mut buffer);
    let peak = buffer
        .channel(0)
        .iter()
        .fold(0.0f32, |peak, value| peak.max(value.abs()));
    assert!((peak - 0.25).abs() < 0.01, "peak {peak}");
}

#[test]
fn the_output_gain_scales_the_result() {
    let mut effect = gain_effect(0.0, -6.0);
    let mut buffer = tone(200.0, 4096, 0.5, 0.5);
    effect.process(&mut buffer);
    let peak = buffer
        .channel(0)
        .iter()
        .fold(0.0f32, |peak, value| peak.max(value.abs()));
    assert!((peak - 0.25).abs() < 0.01, "peak {peak}");
}

#[test]
fn the_two_gains_compose() {
    let mut effect = gain_effect(-6.0, -6.0);
    let mut buffer = tone(200.0, 4096, 0.5, 0.5);
    effect.process(&mut buffer);
    let peak = buffer
        .channel(0)
        .iter()
        .fold(0.0f32, |peak, value| peak.max(value.abs()));
    assert!((peak - 0.125).abs() < 0.01, "peak {peak}");
}
