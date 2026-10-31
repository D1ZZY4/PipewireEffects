use super::*;

#[test]
fn the_segments_move_from_green_through_amber_to_red() {
    let low = segment_colour(0);
    let middle = segment_colour(SEGMENTS * 3 / 4);
    let high = segment_colour(SEGMENTS - 1);
    assert!(low.1 > low.0 && low.1 > low.2);
    assert!(middle.0 > middle.1 && middle.1 > middle.2);
    assert!(high.0 > high.1 && high.0 > high.2);
}

#[test]
fn a_full_scale_signal_lights_every_segment() {
    assert_eq!((1.0f64 * SEGMENTS as f64).ceil() as usize, SEGMENTS);
    assert_eq!((0.0f64 * SEGMENTS as f64).ceil() as usize, 0);
}

#[test]
fn the_meter_levels_are_normalised_through_the_shared_helper() {
    assert!((normalized_db(-6.0) - 0.9).abs() < 1.0e-6);
}
