use super::*;
use crate::param::spec::{ParamSpec, ParamUnit};

#[test]
fn coerce_boolean_into_float() {
    let spec = ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
        .range(0.0, 2.0, 0.0)
        .build();
    assert_eq!(
        ParamValue::Boolean(true).validate(&spec).unwrap(),
        ParamValue::Float(1.0)
    );
}

#[test]
fn reject_non_finite() {
    let spec = ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
        .range(0.0, 2.0, 0.0)
        .build();
    assert!(ParamValue::Float(f32::NAN).validate(&spec).is_err());
}
