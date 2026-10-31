use serde_json::json;

use super::*;

fn parameters(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[test]
fn a_missing_output_gain_uses_the_default() {
    let parameters = parameters(json!({}));
    assert_eq!(number(&parameters, "gain", -3.0), -3.0);
    assert_eq!(text(&parameters, "kind"), "");
}

#[test]
fn level_percent_maps_the_silence_floor_to_nothing() {
    assert_eq!(level_percent(-80.01), 0.0);
    assert!(level_percent(0.0) > 99.9);
    assert!(level_percent(-6.0) > 49.9);
}

#[test]
fn unhandled_parameters_are_reported_once_per_effect() {
    let parameters = parameters(json!({ "bypass": false, "mode": "Upward", "boost-amount": 3.0 }));
    let mut warnings = Vec::new();
    skipped(&parameters, &["bypass"], "compressor", &mut warnings);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("mode"));
    assert!(warnings[0].contains("boost-amount"));
}

#[test]
fn band_blocks_are_not_reported_when_the_effect_consumes_them() {
    let parameters = parameters(json!({ "band0": {}, "left": {} }));
    let mut warnings = Vec::new();
    skipped_with_bands(&parameters, &["left"], "equalizer", &mut warnings);
    assert!(warnings.is_empty());
}

#[test]
fn a_numeric_family_of_keys_is_summarised() {
    let mut value = Map::new();
    for band in 0..13 {
        value.insert(format!("band{band}"), json!({}));
    }
    let parameters = parameters(Value::Object(value));
    let mut warnings = Vec::new();
    skipped(&parameters, &[], "crystalizer", &mut warnings);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("band*"), "{warnings:?}");
    assert!(!warnings[0].contains("band7"), "{warnings:?}");
}

#[test]
fn a_single_numeric_key_keeps_its_name() {
    let parameters = parameters(json!({ "band0": {}, "mode": "Upward" }));
    let mut warnings = Vec::new();
    skipped(&parameters, &[], "crystalizer", &mut warnings);
    assert!(warnings[0].contains("band0"), "{warnings:?}");
    assert!(warnings[0].contains("mode"), "{warnings:?}");
}

#[test]
fn a_handled_parameter_is_not_reported() {
    let parameters = parameters(json!({ "threshold": -20.0 }));
    let mut warnings = Vec::new();
    skipped(&parameters, &["threshold"], "limiter", &mut warnings);
    assert!(warnings.is_empty());
}
