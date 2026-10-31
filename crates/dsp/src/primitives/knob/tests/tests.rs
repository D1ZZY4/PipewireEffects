use super::*;
use pwe_core::{ParamSpec, ParamUnit};

#[test]
fn knob_reads_live_values_and_falls_back() {
    let params = ParamSet::new(vec![
        ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
            .range(-12.0, 12.0, 3.0)
            .build(),
    ]);
    let knob = Knob::new(&params, "gain", 0.0);
    assert_eq!(knob.value(), 3.0);
    params.set_f32("gain", -6.0).unwrap();
    assert_eq!(knob.value(), -6.0);

    let missing = Knob::new(&params, "absent", 7.5);
    assert_eq!(missing.value(), 7.5);
}
