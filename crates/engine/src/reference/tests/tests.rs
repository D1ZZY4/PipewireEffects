use super::*;

#[test]
fn a_pushed_block_comes_back_interleaved() {
    let bus = ReferenceBus::new(1_024);
    let left = [0.1, 0.2, 0.3];
    let right = [-0.1, -0.2, -0.3];
    bus.push(&left, &right);
    let mut out = [9.0; 6];
    assert_eq!(bus.pop(&mut out), 3);
    assert!((out[0] - 0.1).abs() < 1.0e-6);
    assert!((out[1] + 0.1).abs() < 1.0e-6);
    assert!((out[4] - 0.3).abs() < 1.0e-6);
    assert!((out[5] + 0.3).abs() < 1.0e-6);
}

#[test]
fn a_short_read_is_padded_with_silence() {
    let bus = ReferenceBus::new(1_024);
    bus.push(&[0.5], &[0.5]);
    let mut out = [9.0; 8];
    assert_eq!(bus.pop(&mut out), 1);
    assert!((out[0] - 0.5).abs() < 1.0e-6);
    assert!(out[2..].iter().all(|value| *value == 0.0));
}

#[test]
fn a_full_bus_drops_the_new_block_instead_of_overwriting() {
    let bus = ReferenceBus::new(1_024);
    bus.push(&[1.0; 1_024], &[2.0; 1_024]);
    bus.push(&[3.0], &[4.0]);
    let mut out = vec![0.0; 1_024 * 2];
    assert_eq!(bus.pop(&mut out), 1_024);
    assert!((out[2_046] - 1.0).abs() < 1.0e-6);
    assert!((out[2_047] - 2.0).abs() < 1.0e-6);
    assert_eq!(bus.pop(&mut [0.0; 2]), 0);
}

#[test]
fn clear_discards_what_was_written() {
    let bus = ReferenceBus::new(1_024);
    bus.push(&[1.0; 64], &[1.0; 64]);
    bus.clear();
    let mut out = [0.0; 16];
    assert_eq!(bus.pop(&mut out), 0);
}
