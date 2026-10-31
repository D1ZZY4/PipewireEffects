use super::*;

#[test]
fn logarithmic_normalisation_round_trips() {
    let spec = ParamSpec::float("frequency", "Frequency", ParamUnit::Hertz)
        .range(20.0, 20_000.0, 1_000.0)
        .logarithmic()
        .build();
    assert!((spec.normalize(632.46) - 0.5).abs() < 0.01);
    let value = spec.denormalize(0.5);
    assert!((value - 632.46).abs() < 1.0);
    assert!((spec.denormalize(0.0) - 20.0).abs() < 1.0e-3);
    assert!((spec.denormalize(1.0) - 20_000.0).abs() < 1.0e-1);
}

#[test]
fn clamping_respects_bounds() {
    let spec = ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
        .range(-20.0, 20.0, 0.0)
        .build();
    assert!((spec.clamp_f32(100.0) - 20.0).abs() < 1.0e-6);
    assert!((spec.clamp_f32(-100.0) + 20.0).abs() < 1.0e-6);
}

#[test]
fn choice_clamps_to_label_count() {
    let spec = ParamSpec::choice("mode", "Mode", &["a", "b"], 5);
    assert_eq!(spec.default_value(), ParamValue::Choice(1));
    assert_eq!(spec.clamp_f32(9.0), 1.0);
}
