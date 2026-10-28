use super::*;

#[test]
fn decibel_conversions_match_known_values() {
    assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
    assert!((db_to_linear(6.0) - 1.995_262).abs() < 1e-5);
    assert!((db_to_linear(-6.0) - 0.501_187).abs() < 1e-5);
    assert!((db_to_linear(20.0) - 10.0).abs() < 1e-5);
    assert!((linear_to_db(0.5) + 6.0206).abs() < 1e-3);
    assert!((linear_to_db(10.0) - 20.0).abs() < 1e-4);
}

#[test]
fn silence_maps_to_the_floor_rather_than_negative_infinity() {
    assert_eq!(linear_to_db(0.0), SILENCE_FLOOR_DB);
    assert_eq!(linear_to_db(-1.0), SILENCE_FLOOR_DB);
    assert_eq!(linear_to_db(f32::NAN), SILENCE_FLOOR_DB);
    assert_eq!(db_to_linear(-1_000.0), 0.0);
}

#[test]
fn decibel_conversions_round_trip() {
    for db in [-60.0_f32, -12.0, -0.5, 0.0, 3.0, 24.0] {
        assert!((linear_to_db(db_to_linear(db)) - db).abs() < 1e-3);
    }
}

#[test]
fn a_ramp_reaches_its_target_exactly_at_the_end_of_the_block() {
    let mut ramp = Ramp::new(0.0);
    ramp.set_target(1.0);
    ramp.begin(100);
    for _ in 0..100 {
        ramp.next_value();
    }
    ramp.finish();
    assert!((ramp.current() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_ramp_interpolates_monotonically() {
    let mut ramp = Ramp::new(0.0);
    ramp.set_target(1.0);
    ramp.begin(4);
    assert!(ramp.next_value() < ramp.next_value());
    assert!(ramp.next_value() < ramp.next_value());
}

#[test]
fn an_empty_block_leaves_the_ramp_at_its_target() {
    let mut ramp = Ramp::new(0.0);
    ramp.set_target(2.0);
    ramp.begin(0);
    assert!((ramp.current() - 2.0).abs() < f32::EPSILON);
}

#[test]
fn a_smoother_converges_towards_its_target() {
    let mut smoother = Smoother::new(0.0, 1.0, 48_000.0);
    for _ in 0..48_000 {
        smoother.process(1.0);
    }
    assert!(smoother.is_settled(1.0, 1e-3));
}

#[test]
fn advancing_by_a_whole_block_matches_sample_by_sample_stepping() {
    let mut stepped = Smoother::new(0.0, 10.0, 48_000.0);
    let mut blocked = Smoother::new(0.0, 10.0, 48_000.0);
    for _ in 0..480 {
        stepped.process(1.0);
    }
    blocked.advance(1.0, 480);
    assert!((stepped.value() - blocked.value()).abs() < 1e-3);
}

#[test]
fn angular_frequency_is_clamped_away_from_the_extremes() {
    assert!(angular_frequency(0.0, 48_000.0) > 0.0);
    assert!(angular_frequency(1e9, 48_000.0) < PI);
    assert_eq!(angular_frequency(f32::NAN, 48_000.0), 0.0);
}

#[test]
fn subnormal_and_non_finite_values_are_zeroed() {
    assert_eq!(sanitise_value(1e-42), 0.0);
    assert_eq!(sanitise_value(f32::NAN), 0.0);
    assert_eq!(sanitise_value(f32::INFINITY), 0.0);
    assert_eq!(sanitise_value(f32::NEG_INFINITY), 0.0);
    assert_eq!(sanitise_value(1e-3), 1e-3);
    assert_eq!(sanitise_value(0.0), 0.0);
}
