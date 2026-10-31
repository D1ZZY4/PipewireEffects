use super::*;

#[test]
fn format_validation_rejects_nonsense() {
    assert!(AudioFormat::validated(48_000, 2).is_ok());
    assert!(AudioFormat::validated(0, 2).is_err());
    assert!(AudioFormat::validated(48_000, 0).is_err());
}

#[test]
fn interleave_round_trip() {
    let mut buffer = AudioBuffer::new(2, 8);
    buffer.configure(2, 3).unwrap();
    buffer.channel_mut(0).copy_from_slice(&[1.0, 2.0, 3.0]);
    buffer.channel_mut(1).copy_from_slice(&[4.0, 5.0, 6.0]);

    let mut interleaved = [0.0f32; 6];
    assert_eq!(buffer.interleave_into(&mut interleaved), 3);
    assert_eq!(interleaved, [1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);

    let mut other = AudioBuffer::new(2, 8);
    other.configure(2, 3).unwrap();
    assert_eq!(other.deinterleave_from(&interleaved), 3);
    assert_eq!(other.channel(0), &[1.0, 2.0, 3.0]);
    assert_eq!(other.channel(1), &[4.0, 5.0, 6.0]);
}

#[test]
fn configure_rejects_oversized_requests() {
    let mut buffer = AudioBuffer::new(2, 4);
    assert!(buffer.configure(2, 8).is_err());
    assert!(buffer.configure(4, 4).is_err());
}

#[test]
fn peak_and_rms_are_measured() {
    let mut buffer = AudioBuffer::new(2, 4);
    buffer.configure(2, 4).unwrap();
    buffer
        .channel_mut(0)
        .copy_from_slice(&[0.5, -0.5, 0.5, -0.5]);
    buffer
        .channel_mut(1)
        .copy_from_slice(&[0.5, -0.5, 0.5, -0.5]);
    assert!((buffer.peak() - 0.5).abs() < 1.0e-6);
    assert!((buffer.rms() - 0.5).abs() < 1.0e-6);
}
