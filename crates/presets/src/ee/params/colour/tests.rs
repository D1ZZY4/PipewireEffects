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

fn float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
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
    assert_eq!(float(&params, "mode"), 1.0);
    assert_eq!(float(&params, "bits"), 6.0);
    assert!((float(&params, "mix") - 25.0).abs() < 1.0e-9);
    assert!((float(&params, "anti_aliasing") - 75.0).abs() < 1.0e-9);
    assert_eq!(float(&params, "samples"), 4.0);
    assert_eq!(float(&params, "lfo_depth"), 30.0);
    assert!((float(&params, "lfo_rate") - 2.5).abs() < 1.0e-9);
    assert_eq!(float(&params, "dc"), -3.0);
}

#[test]
fn a_boolean_survives_as_a_flag() {
    let (params, _) = convert(json!({ "lfo-active": false }));
    assert_eq!(float(&params, "lfo_active"), 0.0);
}

#[test]
fn an_unknown_mode_is_reported() {
    let (params, warnings) = convert(json!({ "mode": "Square" }));
    assert_eq!(float(&params, "mode"), 0.0);
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
    assert_eq!(float(&params, "cutoff"), 500.0);
    assert_eq!(float(&params, "feed"), 9.5);
}

#[test]
fn a_crossfeed_default_is_the_reference_default() {
    let object = json!({}).as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = crossfeed(&object, &mut warnings);
    assert_eq!(float(&params, "cutoff"), 700.0);
    assert_eq!(float(&params, "feed"), 4.5);
}
