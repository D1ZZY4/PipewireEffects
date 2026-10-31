use super::*;

#[test]
fn rows_are_built_for_every_parameter_kind() {
    let params = ParamSet::new(vec![
        ParamSpec::float("gain", "Gain", pwe_core::ParamUnit::Decibel)
            .range(-12.0, 12.0, 0.0)
            .build(),
        ParamSpec::boolean("mute", "Mute", false),
        ParamSpec::choice("mode", "Mode", &["a", "b"], 0),
    ]);
    assert_eq!(params.specs().len(), 3);
    assert_eq!(params.get("gain"), Some(pwe_core::ParamValue::Float(0.0)));
}
