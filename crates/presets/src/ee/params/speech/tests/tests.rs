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
    let params = speech_processor(&parameters, &mut warnings);
    (params, warnings)
}

#[test]
fn the_stage_switches_and_the_depth_are_transferred() {
    let (params, warnings) = convert(json!({
        "enable-denoise": true,
        "noise-suppression": -24.0,
        "enable-agc": true,
        "vad": { "enable": true }
    }));
    assert_eq!(params.get("denoise"), Some(&PresetValue::Boolean(true)));
    assert_eq!(params.get("agc"), Some(&PresetValue::Boolean(true)));
    assert_eq!(params.get("vad"), Some(&PresetValue::Boolean(true)));
    assert!((as_float(&params, "depth") - 24.0).abs() < 1.0e-9);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_vad_probability_is_reported_because_this_stage_uses_a_level() {
    let (_, warnings) = convert(json!({
        "vad": { "enable": true, "probability-start": 0.9, "probability-continue": 0.5 }
    }));
    let joined = warnings.join(" ");
    assert!(joined.contains("probability-start"), "{joined}");
}

#[test]
fn dereverb_and_the_per_plugin_gains_are_reported() {
    let (_, warnings) = convert(json!({
        "enable-dereverb": true, "input-gain": -3.0, "output-gain": 0.0
    }));
    let joined = warnings.join(" ");
    assert!(joined.contains("enable-dereverb"), "{joined}");
    assert!(joined.contains("input-gain"), "{joined}");
}

#[test]
fn the_reference_defaults_are_honoured_when_a_key_is_missing() {
    let (params, _) = convert(json!({}));
    assert_eq!(params.get("denoise"), Some(&PresetValue::Boolean(false)));
    assert_eq!(params.get("agc"), Some(&PresetValue::Boolean(false)));
    assert_eq!(params.get("vad"), Some(&PresetValue::Boolean(false)));
    assert!((as_float(&params, "depth") - 30.0).abs() < 1.0e-9);
}
