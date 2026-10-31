use serde_json::json;

use super::*;

fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

fn convert(value: Value) -> (BTreeMap<String, PresetValue>, Vec<String>) {
    let parameters = value.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = bass_loudness(&parameters, &mut warnings);
    (params, warnings)
}

#[test]
fn the_three_controls_are_transferred() {
    let (params, warnings) = convert(json!({
        "loudness": -25.0, "output": -4.0, "link": -6.0
    }));
    assert!((as_float(&params, "loudness") + 25.0).abs() < 1.0e-9);
    assert!((as_float(&params, "output") + 4.0).abs() < 1.0e-9);
    assert!((as_float(&params, "link") + 6.0).abs() < 1.0e-9);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_reference_defaults_are_honoured_when_a_key_is_missing() {
    let (params, _) = convert(json!({}));
    assert!((as_float(&params, "loudness") + 3.1).abs() < 1.0e-9);
    assert!((as_float(&params, "output") + 6.0).abs() < 1.0e-9);
    assert!((as_float(&params, "link") + 9.1).abs() < 1.0e-9);
}

#[test]
fn a_per_plugin_gain_without_an_equivalent_is_reported() {
    let (_, warnings) = convert(json!({ "loudness": -10.0, "inputGain": -3.0 }));
    assert!(
        warnings.iter().any(|warning| warning.contains("inputGain")),
        "{warnings:?}"
    );
}
