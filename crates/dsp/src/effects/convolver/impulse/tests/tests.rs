use super::*;

#[test]
fn every_response_has_a_peak_and_a_decaying_tail() {
    for choice in 0..RESPONSES.len() {
        let impulse = response(choice, 48_000, 0);
        let maximum = impulse
            .iter()
            .fold(0.0f32, |current, value| current.max(value.abs()));
        assert!(
            (maximum - 0.5).abs() < 1.0e-5,
            "response {choice}: {maximum}"
        );
        let head: f32 = impulse[..512].iter().map(|value| value.abs()).sum();
        let tail: f32 = impulse[impulse.len() - 512..]
            .iter()
            .map(|value| value.abs())
            .sum();
        assert!(tail < head * 0.5, "response {choice} did not decay");
    }
}

#[test]
fn the_two_channels_differ_so_the_response_has_a_width() {
    let left = response(1, 48_000, 0);
    let right = response(1, 48_000, 1);
    let difference: f32 = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(difference > 0.01, "the channels were identical");
}

#[test]
fn the_duration_follows_the_sample_rate() {
    let at_48 = response(0, 48_000, 0).len();
    let at_96 = response(0, 96_000, 0).len();
    assert!(at_96 > at_48, "{at_96} against {at_48}");
    assert_eq!(response(0, 48_000, 0).len(), at_48);
}

#[test]
fn normalisation_changes_the_energy_not_the_shape() {
    let raw = response(0, 48_000, 0);
    let normalised = normalised(0, 48_000, 0, true);
    assert!((energy(&normalised) - 1.0).abs() < 1.0e-5);
    let ratio = normalised[10] / raw[10];
    for index in 20..100 {
        assert!((normalised[index] / raw[index] - ratio).abs() < 1.0e-4);
    }
}

#[test]
fn every_response_is_bounded_and_allocated_once() {
    for choice in 0..RESPONSES.len() {
        let impulse = response(choice, 192_000, 0);
        assert!(impulse.len() <= MAX_SAMPLES);
        assert!(impulse.iter().all(|value| value.is_finite()));
    }
}
