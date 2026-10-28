use super::*;

#[test]
fn silence_sits_at_the_floor() {
    assert_eq!(amplitude_to_db(0.0), FLOOR_DB);
    assert_eq!(amplitude_to_db(1e-9), FLOOR_DB);
    assert_eq!(amplitude_to_db(f32::NAN), FLOOR_DB);
    assert_eq!(position_of(amplitude_to_db(0.0)), 0.0);
}

#[test]
fn full_scale_reaches_the_end_of_the_track() {
    assert_eq!(position_of(amplitude_to_db(1.0)), 1.0);
}

#[test]
fn the_track_is_uniform_in_decibels() {
    let at_minus_20 = position_of(-20.0);
    let at_minus_40 = position_of(-40.0);
    assert!((at_minus_20 - 2.0 / 3.0).abs() < 1e-9);
    assert!((at_minus_40 - 1.0 / 3.0).abs() < 1e-9);
}

#[test]
fn silence_reads_as_minus_infinity_not_a_number() {
    assert_eq!(format_db(0.0), "  -inf");
    assert_eq!(format_db(1.0), "   0.0");
}

#[test]
fn readings_are_classified_by_level() {
    assert_eq!(reading_classes(1.0), style::STATUS_CLIPPING);
    assert_eq!(reading_classes(0.6), style::STATUS_HOT);
    assert_eq!(reading_classes(0.01), "");
}
