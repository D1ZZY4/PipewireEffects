use super::*;

#[test]
fn zero_db_sits_in_the_middle() {
    assert!((position_of(0.0) - 0.5).abs() < 1e-9);
}

#[test]
fn the_top_of_the_bar_is_the_maximum_boost() {
    assert!(position_of(MAX_DB) < position_of(0.0));
    assert!((position_of(MAX_DB) - 0.0).abs() < 1e-9);
    assert!((position_of(MIN_DB) - 1.0).abs() < 1e-9);
}

#[test]
fn dragging_the_middle_gives_no_gain() {
    assert_eq!(gain_at(48.0, 96), 0.0);
}

#[test]
fn dragging_the_top_gives_the_maximum_boost() {
    assert_eq!(gain_at(0.0, 96), MAX_DB as f32);
    assert_eq!(gain_at(96.0, 96), MIN_DB as f32);
}

#[test]
fn gains_snap_to_a_tenth_of_a_decibel() {
    for y in [0.0, 13.0, 47.0, 61.5, 95.0] {
        let value = gain_at(y, 96);
        assert!(
            (value * 10.0).fract().abs() < 1e-4,
            "{value} is not snapped"
        );
    }
}

#[test]
fn frequencies_are_shortened_above_a_kilohertz() {
    assert_eq!(format_frequency(60.0), "60");
    assert_eq!(format_frequency(8_000.0), "8.0k");
}
