use super::*;

fn spec() -> ParamSpec {
    ParamSpec::float("gain", "Gain", pwe_core::ParamUnit::Decibel)
        .range(-20.0, 20.0, 0.0)
        .build()
}

#[test]
fn coercion_accepts_foreign_types() {
    let spec = spec();
    assert_eq!(
        PresetValue::Boolean(true).coerce(&spec).unwrap(),
        ParamValue::Float(1.0)
    );
    assert_eq!(
        PresetValue::Integer(4).coerce(&spec).unwrap(),
        ParamValue::Float(4.0)
    );
    assert_eq!(
        PresetValue::Text("3.5".into()).coerce(&spec).unwrap(),
        ParamValue::Float(3.5)
    );
}

#[test]
fn coercion_rejects_non_finite_numbers() {
    assert!(PresetValue::Float(f64::INFINITY).coerce(&spec()).is_err());
}

#[test]
fn text_matches_choice_labels() {
    let spec = ParamSpec::choice("mode", "Mode", &["Split", "Wideband"], 0);
    assert_eq!(
        PresetValue::Text("wideband".into()).coerce(&spec).unwrap(),
        ParamValue::Choice(1)
    );
}

#[test]
fn round_trip_through_json() {
    let mut preset = Preset::new("Demo");
    let mut params = BTreeMap::new();
    params.insert("gain".to_owned(), PresetValue::Float(0.5));
    preset.output.effects.push(EffectEntry {
        kind: "gain".to_owned(),
        enabled: true,
        params,
    });
    let text = serde_json::to_string(&preset).unwrap();
    let parsed: Preset = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed, preset);
}

#[test]
fn an_older_document_without_an_input_section_still_loads() {
    let text = "{\"schema_version\":1,\"name\":\"Old\",\"output\":{\"effects\":[]}}";
    let preset: Preset = serde_json::from_str(text).expect("should parse");
    assert!(preset.input.effects.is_empty());
    assert!(preset.pipeline(PipelineKind::Output).effects.is_empty());
}

#[test]
fn names_are_validated() {
    assert!(validate_name("Night mode").is_ok());
    assert!(validate_name("../escape").is_err());
    assert!(validate_name("a/b").is_err());
    assert!(validate_name(" hidden").is_err());
    assert!(validate_name(".hidden").is_err());
    assert!(validate_name("").is_err());
}

#[test]
fn newer_schemas_are_rejected() {
    let mut preset = Preset::new("Future");
    preset.schema_version = SCHEMA_VERSION + 1;
    assert!(preset.validate().is_err());
}
