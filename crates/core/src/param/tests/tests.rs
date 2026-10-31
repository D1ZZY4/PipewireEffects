use crate::param::{ParamSet, ParamSpec, ParamUnit, ParamValue};

#[test]
fn the_set_publishes_the_specifications_it_was_built_from() {
    let specs = vec![
        ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .build(),
        ParamSpec::boolean("enabled", "Enabled", true),
        ParamSpec::choice("mode", "Mode", &["stereo", "mono"], 0),
    ];
    let set = ParamSet::new(specs);
    assert_eq!(set.len(), 3);
    assert!(!set.is_empty());
    assert_eq!(
        set.spec("gain").map(|spec| spec.unit),
        Some(ParamUnit::Decibel)
    );
    assert_eq!(set.get("enabled"), Some(ParamValue::Boolean(true)));
    assert_eq!(set.get_f32("missing"), None);
    assert_eq!(set.entries().len(), 3);
}
