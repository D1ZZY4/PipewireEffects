use super::*;

#[test]
fn gain_round_trip() {
    for db in [-60.0, -6.0, 0.0, 6.0, 24.0] {
        assert!((gain_to_db(db_to_gain(db)) - db).abs() < 1.0e-3);
    }
}

#[test]
fn sanitize_flushes_non_finite() {
    assert_eq!(sanitize(f32::NAN), 0.0);
    assert_eq!(sanitize(f32::INFINITY), 0.0);
    assert_eq!(sanitize(1.0e-30), 0.0);
    assert_eq!(sanitize(0.25), 0.25);
}

#[test]
fn smoothed_reaches_target() {
    let mut smoothed = Smoothed::new(0.0);
    smoothed.set_time_constant(1.0, 48_000);
    smoothed.set_target(1.0);
    for _ in 0..48_000 {
        smoothed.advance();
    }
    assert!((smoothed.current() - 1.0).abs() < 1.0e-3);
}
