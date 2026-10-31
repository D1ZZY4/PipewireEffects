use super::*;

fn stereo(left: f32, right: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer.channel_mut(0).fill(left);
    buffer.channel_mut(1).fill(right);
    buffer
}

fn prepared(params: ParamSet) -> StereoTools {
    let mut effect = StereoTools::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

#[test]
fn zero_width_collapses_to_mono() {
    let params = ParamSet::new(specs());
    params.set_f32("stereo_width", 0.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(1.0, -1.0, 8);
    effect.process(&mut buffer);
    assert!(buffer.channel(0)[0].abs() < 1.0e-6);
    assert!(buffer.channel(1)[0].abs() < 1.0e-6);
}

#[test]
fn double_width_amplifies_the_side_channel() {
    let params = ParamSet::new(specs());
    params.set_f32("stereo_width", 200.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(1.0, -1.0, 8);
    effect.process(&mut buffer);
    assert!(
        (buffer.channel(0)[0] - 2.0).abs() < 1.0e-5,
        "{}",
        buffer.channel(0)[0]
    );
    assert!((buffer.channel(1)[0] + 2.0).abs() < 1.0e-5);
}

#[test]
fn the_middle_and_side_levels_are_independent() {
    let params = ParamSet::new(specs());
    params.set_f32("middle_level", -20.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.5, 0.5, 8);
    effect.process(&mut buffer);
    assert!(buffer.channel(0)[0] < 0.06, "{}", buffer.channel(0)[0]);
    let params = ParamSet::new(specs());
    params.set_f32("side_level", -20.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.5, -0.5, 8);
    effect.process(&mut buffer);
    assert!(buffer.channel(0)[0] < 0.06, "{}", buffer.channel(0)[0]);
    let params = ParamSet::new(specs());
    params.set_f32("side_level", 6.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.5, -0.5, 8);
    effect.process(&mut buffer);
    assert!(buffer.channel(0)[0] > 0.9, "{}", buffer.channel(0)[0]);
}

#[test]
fn a_mode_that_sums_the_channels_makes_them_equal() {
    let params = ParamSet::new(specs());
    params.set_choice("mode", 5).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.75, 0.25, 8);
    effect.process(&mut buffer);
    assert!((buffer.channel(0)[0] - 0.5).abs() < 1.0e-6);
    assert!((buffer.channel(1)[0] - 0.5).abs() < 1.0e-6);
}

#[test]
fn a_mode_that_swaps_the_channels_reverses_them() {
    let params = ParamSet::new(specs());
    params.set_choice("mode", 6).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.75, 0.25, 8);
    effect.process(&mut buffer);
    assert!((buffer.channel(0)[0] - 0.25).abs() < 1.0e-6);
    assert!((buffer.channel(1)[0] - 0.75).abs() < 1.0e-6);
}

#[test]
fn the_mid_side_modes_round_trip() {
    let params = ParamSet::new(specs());
    params.set_choice("mode", 1).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.75, 0.25, 8);
    effect.process(&mut buffer);
    assert!((buffer.channel(0)[0] - 0.5).abs() < 1.0e-6);
    assert!((buffer.channel(1)[0] - 0.25).abs() < 1.0e-6);

    let params = ParamSet::new(specs());
    params.set_choice("mode", 2).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.5, 0.25, 8);
    effect.process(&mut buffer);
    assert!((buffer.channel(0)[0] - 0.75).abs() < 1.0e-6);
    assert!((buffer.channel(1)[0] - 0.25).abs() < 1.0e-6);
}

#[test]
fn balance_attenuates_the_opposite_channel() {
    let params = ParamSet::new(specs());
    params.set_f32("balance_out", 100.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.5, 0.5, 8);
    effect.process(&mut buffer);
    assert!(buffer.channel(0)[0].abs() < 1.0e-6);
}

#[test]
fn the_phase_and_mute_switches_act_on_one_channel() {
    let params = ParamSet::new(specs());
    params.set_bool("phase_right", true).unwrap();
    params.set_bool("mute_left", true).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(0.5, 0.25, 8);
    effect.process(&mut buffer);
    assert!(buffer.channel(0)[0].abs() < 1.0e-6);
    assert!((buffer.channel(1)[0] + 0.25).abs() < 1.0e-6);
}

#[test]
fn soft_clip_holds_a_hot_signal_below_unity() {
    let params = ParamSet::new(specs());
    params.set_bool("softclip", true).unwrap();
    let mut effect = prepared(params);
    let mut buffer = stereo(4.0, 4.0, 8);
    effect.process(&mut buffer);
    assert!(buffer.channel(0)[0] < 1.0, "{}", buffer.channel(0)[0]);
    assert!(buffer.channel(0)[0] > 0.9, "{}", buffer.channel(0)[0]);
}

#[test]
fn the_delay_shifts_the_right_channel_in_time() {
    let params = ParamSet::new(specs());
    params.set_f32("delay", 1.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = AudioBuffer::new(2, 256);
    buffer.configure(2, 256).unwrap();
    buffer.channel_mut(0)[0] = 1.0;
    buffer.channel_mut(1)[0] = 1.0;
    effect.process(&mut buffer);
    let right_peak = buffer
        .channel(1)
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(index, _)| index)
        .unwrap();
    assert!(right_peak.abs_diff(48) < 4, "right peak at {right_peak}");
}
