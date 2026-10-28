use super::*;

#[test]
fn file_stems_are_normalised() {
    assert_eq!(file_stem("Bass Boost").unwrap(), "bass-boost");
    assert_eq!(file_stem("  Vocal   Clarity!! ").unwrap(), "vocal-clarity");
    assert_eq!(file_stem("Night/Mode").unwrap(), "night-mode");
    assert_eq!(file_stem("Already-good").unwrap(), "already-good");
    assert!(file_stem("!!").is_err());
    assert!(file_stem("").is_err());
}

#[test]
fn named_presets_round_trip_within_the_schema_version() {
    let preset = Preset::new("Test", "A test.");
    assert!(preset.validate().is_ok());

    let mut newer = preset;
    newer.schema_version = SCHEMA_VERSION + 1;
    assert!(matches!(
        newer.validate(),
        Err(PresetError::UnsupportedSchema { .. })
    ));
}

#[test]
fn a_missing_schema_version_defaults_to_the_current_one() {
    let preset: Preset = serde_json::from_str(r#"{ "name": "Bare" }"#).unwrap();
    assert_eq!(preset.schema_version, SCHEMA_VERSION);
    assert!(preset.output.effects.is_empty());
    assert!(!preset.output.bypassed);
}

#[test]
fn effects_are_enabled_unless_stated_otherwise() {
    let preset: Preset = serde_json::from_str(
        r#"{ "name": "Bare", "output": { "effects": [ { "kind": "gain" } ] } }"#,
    )
    .unwrap();
    assert!(preset.output.effects[0].enabled);
    assert!(preset.output.effects[0].parameters.is_empty());
}

#[test]
fn parameter_values_keep_their_json_shape() {
    let preset: Preset = serde_json::from_str(
        r#"{ "name": "P", "output": { "effects": [ { "kind": "gain",
             "parameters": { "gain": -3.5, "mute": true, "mode": 2 } } ] } }"#,
    )
    .unwrap();
    let parameters = &preset.output.effects[0].parameters;
    assert_eq!(parameters["gain"], ParamRepr::Number(-3.5));
    assert_eq!(parameters["mute"], ParamRepr::Bool(true));
    assert_eq!(parameters["mode"], ParamRepr::Integer(2));

    let json = serde_json::to_string(&preset).unwrap();
    assert!(json.contains(r#""gain":-3.5"#));
    assert!(json.contains(r#""mute":true"#));
    assert!(json.contains(r#""mode":2"#));
}

#[test]
fn recipes_survive_a_conversion_round_trip() {
    let recipe = ChainRecipe {
        bypassed: true,
        effects: vec![EffectRecipe {
            kind: "parametric-eq".to_owned(),
            enabled: false,
            values: vec![
                ("band1_gain".to_owned(), ParamRepr::Number(6.0)),
                ("band1_enabled".to_owned(), ParamRepr::Bool(true)),
            ],
        }],
    };
    let preset = Preset::from_recipe("Round Trip", "", &recipe);

    let mut expected = recipe;
    expected.effects[0]
        .values
        .sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(preset.to_recipe(), expected);
}
