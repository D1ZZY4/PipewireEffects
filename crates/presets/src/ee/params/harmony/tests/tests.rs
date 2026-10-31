use serde_json::json;

use super::*;

type Converter = fn(&Map<String, Value>, &mut Vec<String>) -> BTreeMap<String, PresetValue>;

fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

fn convert(effect: Converter, value: Value) -> (BTreeMap<String, PresetValue>, Vec<String>) {
    let parameters = value.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = effect(&parameters, &mut warnings);
    (params, warnings)
}

#[test]
fn the_correction_speed_and_tuning_are_transferred() {
    let (params, warnings) = convert(autotune, json!({ "correction": 40.0, "tuning": 432.0 }));
    assert!((as_float(&params, "speed") - 40.0).abs() < 1.0e-9);
    assert!((as_float(&params, "reference") - 432.0).abs() < 1.0e-9);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_scale_mask_and_the_gains_are_reported() {
    let (_, warnings) = convert(
        autotune,
        json!({ "mode": "LFO", "note-c": true, "bend-range": 3.0, "input-gain": -2.0 }),
    );
    let joined = warnings.join(" ");
    assert!(joined.contains("note-c"), "{joined}");
    assert!(joined.contains("bend-range"), "{joined}");
}

#[test]
fn the_harmonizer_intervals_and_levels_are_transferred() {
    let (params, warnings) = convert(
        harmonizer,
        json!({
            "interval-left": 5.0, "interval-right": -12.0,
            "gain-left": 0.0, "gain-right": -6.0, "dry-wet": -3.0
        }),
    );
    assert!((as_float(&params, "interval1") - 5.0).abs() < 1.0e-9);
    assert!((as_float(&params, "interval2") + 12.0).abs() < 1.0e-9);
    assert!((as_float(&params, "level1") - 100.0).abs() < 0.1);
    assert!((as_float(&params, "level2") - 50.1).abs() < 0.6);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_harmonizer_chord_settings_are_reported() {
    let (_, warnings) = convert(harmonizer, json!({ "chord": "Major", "note": "C4" }));
    let joined = warnings.join(" ");
    assert!(joined.contains("chord"), "{joined}");
    assert!(joined.contains("note"), "{joined}");
}
