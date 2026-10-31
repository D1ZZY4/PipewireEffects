use super::*;

fn effect(delay: f32, decay: f32, phantom: f32) -> Crosstalk {
    let params = ParamSet::new(specs());
    params.set_f32("delay", delay).unwrap();
    params.set_f32("decay", decay).unwrap();
    params.set_f32("phantom_center", phantom).unwrap();
    let mut effect = Crosstalk::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

fn one_sided(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for index in 0..frames {
        let value = amplitude * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
        buffer.channel_mut(1)[index] = value;
    }
    buffer
}

#[test]
fn a_mono_signal_keeps_both_channels_equal() {
    let mut effect = effect(313.0, -2.0, 0.0);
    let mut buffer = one_sided(0.4, 8_192);
    for index in 0..buffer.frames() {
        let value = buffer.channel(1)[index];
        buffer.channel_mut(0)[index] = value;
    }
    effect.process(&mut buffer);
    for index in 0..buffer.frames() {
        let left = buffer.channel(0)[index];
        let right = buffer.channel(1)[index];
        assert!((left - right).abs() < 1.0e-5, "{left} {right} at {index}");
    }
}

#[test]
fn silence_stays_silent() {
    let mut effect = effect(313.0, 0.0, 0.0);
    let mut buffer = AudioBuffer::new(2, 4_096);
    buffer.configure(2, 4_096).unwrap();
    effect.process(&mut buffer);
    assert_eq!(buffer.peak(), 0.0);
}

#[test]
fn a_signal_on_one_side_leaks_a_delayed_copy_to_the_other() {
    let mut effect = effect(313.0, 0.0, 0.0);
    let mut buffer = one_sided(0.5, 9_600);
    effect.process(&mut buffer);
    let leaked = buffer
        .channel(0)
        .iter()
        .fold(0.0f32, |peak, value| peak.max(value.abs()));
    assert!(leaked > 0.02, "no crossfeed appeared: {leaked}");
}

#[test]
fn the_leak_is_the_configured_delay_late() {
    let mut effect = effect(400.0, 0.0, 0.0);
    let mut buffer = AudioBuffer::new(2, 2048);
    buffer.configure(2, 2048).unwrap();
    buffer.channel_mut(1)[0] = 1.0;
    effect.process(&mut buffer);
    let expected = (400.0e-6f32 * 48_000.0).round() as usize;
    let peak_index = buffer
        .channel(0)
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(index, _)| index)
        .unwrap();
    assert!(
        peak_index.abs_diff(expected) < 4,
        "{peak_index} against {expected}"
    );
}

#[test]
fn the_phantom_centre_mode_keeps_a_difference_signal_intact() {
    let mut effect = effect(313.0, -2.0, 1.0);
    let mut buffer = one_sided(0.5, 4096);
    effect.process(&mut buffer);
    for sample in buffer.channel(0) {
        assert!(sample.is_finite() && sample.abs() < 2.0, "{sample}");
    }
}

#[test]
fn extreme_settings_stay_bounded() {
    let mut effect = effect(500.0, 0.0, 0.0);
    let mut buffer = one_sided(0.9, 8192);
    effect.process(&mut buffer);
    for channel in 0..2 {
        for sample in buffer.channel(channel) {
            assert!(sample.is_finite() && sample.abs() < 4.0, "{sample}");
        }
    }
}

fn gained(input_gain: f32, output_gain: f32) -> Crosstalk {
    let params = ParamSet::new(specs());
    params.set_f32("input_gain", input_gain).unwrap();
    params.set_f32("output_gain", output_gain).unwrap();
    let mut effect = Crosstalk::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

fn right_impulse(effect: &mut Crosstalk) -> f32 {
    let mut buffer = AudioBuffer::new(2, 8);
    buffer.configure(2, 8).unwrap();
    buffer.channel_mut(1)[0] = 1.0;
    effect.process(&mut buffer);
    buffer.channel(1)[0]
}

#[test]
fn the_input_gain_scales_the_input() {
    let value = right_impulse(&mut gained(-6.0, 0.0));
    assert!((value - 0.501).abs() < 0.01, "{value}");
}

#[test]
fn the_output_gain_scales_the_result() {
    let value = right_impulse(&mut gained(0.0, -6.0));
    assert!((value - 0.501).abs() < 0.01, "{value}");
}

#[test]
fn the_two_gains_compose() {
    let value = right_impulse(&mut gained(-6.0, -6.0));
    assert!((value - 0.251).abs() < 0.01, "{value}");
}
