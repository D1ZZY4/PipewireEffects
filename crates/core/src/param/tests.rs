use crate::error::CoreError;

use super::*;

fn gain_spec() -> ParamSpec {
    ParamSpec::float("gain", "Gain", -60.0, 24.0, 0.0, 0.1, ParamUnit::Decibel)
}

#[test]
fn defaults_are_applied_on_construction() {
    let set = ParamSet::new(vec![gain_spec()]);
    assert_eq!(set.value(0), Some(ParamValue::Float(0.0)));
}

#[test]
fn float_values_are_clamped_into_range() {
    let set = ParamSet::new(vec![gain_spec()]);
    set.set_f32(0, 1_000.0);
    assert_eq!(set.value_f32(0), 24.0);
    set.set_f32(0, -1_000.0);
    assert_eq!(set.value_f32(0), -60.0);
}

#[test]
fn non_finite_values_are_ignored() {
    let set = ParamSet::new(vec![gain_spec()]);
    set.set_f32(0, 6.0);
    set.set_f32(0, f32::NAN);
    assert_eq!(set.value_f32(0), 6.0);
}

#[test]
fn type_mismatches_are_rejected() {
    let set = ParamSet::new(vec![gain_spec()]);
    let error = set.set(0, ParamValue::Bool(true));
    assert!(matches!(error, Err(CoreError::ValueTypeMismatch { .. })));
}

#[test]
fn unknown_parameter_is_reported() {
    let set = ParamSet::new(vec![gain_spec()]);
    let error = set.set_by_id("nope", ParamValue::Float(1.0));
    assert!(matches!(error, Err(CoreError::UnknownParameter { .. })));
}

#[test]
fn logarithmic_round_trip_is_stable() {
    let spec = ParamSpec::float_log(
        "freq",
        "Frequency",
        20.0,
        20_000.0,
        1_000.0,
        ParamUnit::Hertz,
    );
    for probe in [20.0_f32, 100.0, 1_000.0, 8_000.0, 20_000.0] {
        let position = spec.kind.normalise(probe);
        let back = spec.kind.denormalise(position);
        assert!(
            (back - probe).abs() / probe < 1e-3,
            "round trip failed for {probe}: got {back}"
        );
    }
}

#[test]
fn hertz_is_formatted_with_a_kilohertz_suffix_when_long() {
    let spec = ParamSpec::float(
        "freq",
        "Frequency",
        20.0,
        20_000.0,
        440.0,
        1.0,
        ParamUnit::Hertz,
    );
    assert_eq!(spec.kind.format_value(ParamValue::Float(440.0)), "440 Hz");
    assert_eq!(
        spec.kind.format_value(ParamValue::Float(1_200.0)),
        "1.20 kHz"
    );
}

#[test]
fn enumeration_indices_are_clamped() {
    let spec = ParamSpec::enumerate("mode", "Mode", &["Low", "Mid", "High"], 1);
    assert_eq!(spec.kind.clamp_f32(99.0), 2.0);
    assert_eq!(spec.kind.clamp_f32(-3.0), 0.0);
}

#[test]
fn atomic_params_are_shared_between_handles() {
    let set = ParamSet::new(vec![gain_spec()]);
    let cloned = set.values()[0].clone();
    set.set_f32(0, 3.0);
    assert_eq!(cloned.get(), 3.0);
    assert!(cloned.ptr_eq(&set.values()[0]));
}
