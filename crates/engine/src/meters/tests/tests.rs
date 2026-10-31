use super::*;

#[test]
fn a_loudness_meter_is_absent_until_it_measures() {
    let meters = Meters::default();
    let snapshot = meters.snapshot();
    assert_eq!(snapshot.loudness_momentary_lufs, f32::NEG_INFINITY);
    assert_eq!(snapshot.loudness_short_term_lufs, f32::NEG_INFINITY);
    assert_eq!(snapshot.loudness_integrated_lufs, f32::NEG_INFINITY);
}

#[test]
fn a_published_loudness_round_trips() {
    let meters = Meters::default();
    meters.publish_loudness(Some(-18.5), Some(-16.25), Some(-14.0));
    let snapshot = meters.snapshot();
    assert!((snapshot.loudness_momentary_lufs + 18.5).abs() < 1.0e-6);
    assert!((snapshot.loudness_short_term_lufs + 16.25).abs() < 1.0e-6);
    assert!((snapshot.loudness_integrated_lufs + 14.0).abs() < 1.0e-6);
}

#[test]
fn slot_levels_are_published_in_decibels() {
    let meters = Meters::default();
    meters.publish_slot_levels(&[0.5, 1.0, 0.0]);
    let snapshot = meters.snapshot();
    assert!((snapshot.slot_peak_db[0] + 6.0206).abs() < 0.01);
    assert!(snapshot.slot_peak_db[1].abs() < 1.0e-6);
    assert_eq!(snapshot.slot_peak_db[2], f32::NEG_INFINITY);
    assert_eq!(snapshot.slot_peak_db[MAX_EFFECTS - 1], f32::NEG_INFINITY);
}
