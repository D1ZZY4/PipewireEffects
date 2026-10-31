use super::*;
use crate::param::spec::{ParamSpec, ParamUnit};

fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .build(),
        ParamSpec::boolean("enabled", "Enabled", true),
        ParamSpec::choice("mode", "Mode", &["stereo", "mono"], 0),
    ]
}

#[test]
fn clone_shares_the_same_cells() {
    let set = ParamSet::new(specs());
    let mirror = set.clone();
    set.set_f32("gain", 6.0).unwrap();
    assert_eq!(mirror.get_f32("gain"), Some(6.0));
}

#[test]
fn values_are_clamped_on_write() {
    let set = ParamSet::new(specs());
    assert_eq!(set.set_f32("gain", 999.0).unwrap(), 20.0);
    assert_eq!(set.set_f32("gain", -999.0).unwrap(), -20.0);
}

#[test]
fn unknown_parameter_is_reported() {
    let set = ParamSet::new(specs());
    assert!(matches!(
        set.set_f32("missing", 1.0),
        Err(CoreError::UnknownParameter(_))
    ));
    assert!(set.get("missing").is_none());
}

#[test]
fn handles_observe_live_updates() {
    let set = ParamSet::new(specs());
    let handle = set.handle("gain").unwrap();
    assert!(set.handle("nope").is_none());
    set.set_f32("gain", -3.0).unwrap();
    assert_eq!(handle.get_f32(), -3.0);
}

#[test]
fn reset_restores_defaults() {
    let set = ParamSet::new(specs());
    set.set_f32("gain", 10.0).unwrap();
    set.set_bool("enabled", false).unwrap();
    set.reset();
    assert_eq!(set.get("gain"), Some(ParamValue::Float(0.0)));
    assert_eq!(set.get("enabled"), Some(ParamValue::Boolean(true)));
    assert_eq!(set.get("mode"), Some(ParamValue::Choice(0)));
}
