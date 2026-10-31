use super::*;

#[test]
fn a_published_frame_is_returned_unchanged() {
    let spectrum = Spectrum::new();
    let mut values = [FLOOR_DB; BANDS];
    values[3] = -24.5;
    spectrum.publish(&values);
    let snapshot = spectrum.snapshot();
    assert_eq!(snapshot[3], -24.5);
    assert_eq!(snapshot[0], FLOOR_DB);
}

#[test]
fn clearing_returns_every_band_to_the_floor() {
    let spectrum = Spectrum::new();
    let values = [0.0f32; BANDS];
    spectrum.publish(&values);
    spectrum.clear();
    assert!(spectrum.snapshot().iter().all(|value| *value <= FLOOR_DB));
}
