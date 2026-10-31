use super::*;

fn stereo(values: [f32; 2]) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, 2);
    buffer.configure(2, 1).unwrap();
    buffer.channel_mut(0)[0] = values[0];
    buffer.channel_mut(1)[0] = values[1];
    buffer
}

#[test]
fn gain_scales_every_channel() {
    let mut buffer = stereo([1.0, -1.0]);
    apply_gain(&mut buffer, 0.5);
    assert_eq!(buffer.channel(0)[0], 0.5);
    assert_eq!(buffer.channel(1)[0], -0.5);
}

#[test]
fn mean_square_matches_the_signal_power() {
    let buffer = stereo([0.5, -0.5]);
    assert!((mean_square(&buffer) - 0.25).abs() < 1.0e-6);
}
