use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;
use crate::model::PresetValue;

fn convert(value: Value) -> (BTreeMap<String, PresetValue>, Vec<String>) {
    let parameters = value.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = crusher(&parameters, &mut warnings);
    (params, warnings)
}

fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

#[test]
fn the_reference_labels_and_keys_reach_the_parameters() {
    let (params, warnings) = convert(json!({
        "bypass": false,
        "mode": "Logarithmic",
        "bit-reduction": 6.0,
        "morph": 0.25,
        "anti-aliasing": 0.75,
        "sample-reduction": 4.0,
        "lfo-active": true,
        "lfo-range": 30.0,
        "lfo-rate": 2.5,
        "dc": -3.0
    }));
    assert!(
        warnings.iter().all(|warning| !warning.contains("crusher:")),
        "{warnings:?}"
    );
    assert_eq!(as_float(&params, "mode"), 1.0);
    assert_eq!(as_float(&params, "bits"), 6.0);
    assert!((as_float(&params, "mix") - 25.0).abs() < 1.0e-9);
    assert!((as_float(&params, "anti_aliasing") - 75.0).abs() < 1.0e-9);
    assert_eq!(as_float(&params, "samples"), 4.0);
    assert_eq!(as_float(&params, "lfo_depth"), 30.0);
    assert!((as_float(&params, "lfo_rate") - 2.5).abs() < 1.0e-9);
    assert_eq!(as_float(&params, "dc"), -3.0);
}

#[test]
fn a_boolean_survives_as_a_flag() {
    let (params, _) = convert(json!({ "lfo-active": false }));
    assert_eq!(as_float(&params, "lfo_active"), 0.0);
}

#[test]
fn an_unknown_mode_is_reported() {
    let (params, warnings) = convert(json!({ "mode": "Square" }));
    assert_eq!(as_float(&params, "mode"), 0.0);
    assert!(warnings.iter().any(|warning| warning.contains("Square")));
}

#[test]
fn the_plugin_gain_pair_is_reported() {
    let (_, warnings) = convert(json!({ "input-gain": 2.0, "output-gain": -1.0 }));
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("input-gain"), "{warnings:?}");
    assert!(warnings[0].contains("output-gain"), "{warnings:?}");
}

#[test]
fn the_crossfeed_cutoff_and_feed_are_transferred() {
    let parameters = json!({ "fcut": 500.0, "feed": 9.5 });
    let object = parameters.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = crossfeed(&object, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(as_float(&params, "cutoff"), 500.0);
    assert_eq!(as_float(&params, "feed"), 9.5);
}

#[test]
fn a_crossfeed_default_is_the_reference_default() {
    let object = json!({}).as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = crossfeed(&object, &mut warnings);
    assert_eq!(as_float(&params, "cutoff"), 700.0);
    assert_eq!(as_float(&params, "feed"), 4.5);
}

#[test]
fn the_noise_reduction_gains_the_gate_and_its_threshold_are_transferred() {
    let object = json!({
        "inputGain": 1.5,
        "outputGain": -2.5,
        "enableVad": true,
        "vadThres": 62.0,
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    let mut warnings = Vec::new();
    let params = noise_reduction(&object, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(as_float(&params, "input_gain"), 1.5);
    assert_eq!(as_float(&params, "output_gain"), -2.5);
    assert_eq!(as_float(&params, "vad"), 1.0);
    assert_eq!(as_float(&params, "vad_threshold"), 62.0);
}

#[test]
fn the_noise_reduction_wet_level_becomes_the_mix() {
    let object = json!({ "wet": -6.0 })
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut warnings = Vec::new();
    let params = noise_reduction(&object, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!((as_float(&params, "mix") - 50.1).abs() < 0.2, "{params:?}");
    let silent = json!({ "wet": -80.01 })
        .as_object()
        .cloned()
        .unwrap_or_default();
    let params = noise_reduction(&silent, &mut Vec::new());
    assert_eq!(as_float(&params, "mix"), 0.0);
}

#[test]
fn the_noise_reduction_settings_without_a_control_are_reported() {
    let object = json!({
        "modelName": "custom",
        "useStandardModel": false,
        "release": 500.0,
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    let mut warnings = Vec::new();
    let params = noise_reduction(&object, &mut warnings);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("modelName"), "{warnings:?}");
    assert!(warnings[0].contains("release"), "{warnings:?}");
    assert!(warnings[0].contains("useStandardModel"), "{warnings:?}");
    assert!(params.contains_key("vad"));
}

#[test]
fn the_noise_reduction_defaults_come_from_the_reference() {
    let object = json!({}).as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = noise_reduction(&object, &mut warnings);
    assert_eq!(as_float(&params, "output_gain"), 0.0);
    assert!(as_float(&params, "mix") > 99.9);
    assert_eq!(as_float(&params, "vad"), 0.0);
    assert_eq!(as_float(&params, "vad_threshold"), 50.0);
}
