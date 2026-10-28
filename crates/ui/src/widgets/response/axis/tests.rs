use super::*;

#[test]
fn frequency_mapping_round_trips() {
    for hz in [20.0, 100.0, 1_000.0, 10_000.0, 20_000.0] {
        let back = frequency_of(frequency_position(hz));
        assert!((back - hz).abs() / hz < 1e-9, "{hz} became {back}");
    }
}

#[test]
fn a_decade_is_the_same_distance_everywhere() {
    let first = frequency_position(400.0) - frequency_position(40.0);
    let second = frequency_position(4_000.0) - frequency_position(400.0);
    assert!((first - second).abs() < 1e-9, "{first} versus {second}");
}

#[test]
fn the_axis_spans_twenty_hertz_to_twenty_kilohertz() {
    assert!((frequency_position(MIN_HZ) - 0.0).abs() < 1e-9);
    assert!((frequency_position(MAX_HZ) - 1.0).abs() < 1e-9);
    assert_eq!(frequency_position(5.0), 0.0);
    assert_eq!(frequency_position(40_000.0), 1.0);
}

#[test]
fn gain_mapping_round_trips_and_inverts() {
    for db in [-24.0, -12.0, 0.0, 12.0, 24.0] {
        assert!((gain_of(gain_position(db)) - db).abs() < 1e-9);
    }
    assert!(gain_position(MAX_DB) < gain_position(MIN_DB));
    assert!((gain_position(0.0) - 0.5).abs() < 1e-9);
}

#[test]
fn out_of_range_gains_are_clamped() {
    assert_eq!(gain_position(100.0), 0.0);
    assert_eq!(gain_position(-100.0), 1.0);
}
