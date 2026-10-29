use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::model::PresetValue;

use super::{autogain, compressor, deesser, expander, gate, limiter};

type Converter = fn(&Map<String, Value>, &mut Vec<String>) -> BTreeMap<String, PresetValue>;

fn convert(effect: Converter, value: Value) -> (BTreeMap<String, PresetValue>, Vec<String>) {
    let parameters = value.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = effect(&parameters, &mut warnings);
    (params, warnings)
}

fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

#[test]
fn the_compressor_knee_is_a_width_and_the_wet_level_is_a_mix() {
    let (params, _) = convert(
        compressor,
        json!({ "knee": -6.0, "wet": 0.0, "threshold": -20.0 }),
    );
    assert!((as_float(&params, "knee") - 6.0).abs() < 1.0e-9);
    assert!((as_float(&params, "mix") - 100.0).abs() < 0.1);
    assert!((as_float(&params, "threshold") + 20.0).abs() < 1.0e-9);
}

#[test]
fn a_dry_compressor_becomes_a_zero_mix() {
    let (params, _) = convert(compressor, json!({ "wet": -80.01 }));
    assert_eq!(as_float(&params, "mix"), 0.0);
}

#[test]
fn the_compressor_detector_follows_the_sidechain() {
    let (rms, _) = convert(compressor, json!({ "sidechain": { "mode": "RMS" } }));
    assert_eq!(as_float(&rms, "detector"), 1.0);
    let (peak, _) = convert(compressor, json!({ "sidechain": { "mode": "Peak" } }));
    assert_eq!(as_float(&peak, "detector"), 0.0);
}

#[test]
fn an_extra_sidechain_setting_is_reported() {
    let (_, warnings) = convert(
        compressor,
        json!({ "sidechain": { "mode": "RMS", "lookahead": 1.0, "preamp": 0.0 } }),
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("sidechain.lookahead")),
        "{warnings:?}"
    );
}

#[test]
fn an_unsupported_sidechain_mode_is_reported() {
    let (params, warnings) = convert(compressor, json!({ "sidechain": { "mode": "Low-Pass" } }));
    assert_eq!(as_float(&params, "detector"), 0.0);
    assert!(
        warnings.iter().any(|warning| warning.contains("Low-Pass")),
        "{warnings:?}"
    );
}

#[test]
fn the_limiter_takes_its_ceiling_from_the_threshold() {
    let (params, _) = convert(limiter, json!({ "threshold": -6.0, "release": 5.0 }));
    assert_eq!(as_float(&params, "limit"), -6.0);
    assert_eq!(as_float(&params, "release"), 5.0);
}

#[test]
fn the_gate_takes_its_threshold_from_the_curve_threshold() {
    let (params, warnings) = convert(
        gate,
        json!({ "curve-threshold": -30.0, "reduction": -40.0, "wet": 0.0 }),
    );
    assert_eq!(as_float(&params, "threshold"), -30.0);
    assert_eq!(as_float(&params, "range"), 40.0);
    assert!((as_float(&params, "mix") - 100.0).abs() < 0.1);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_gate_reports_reverse_mode_and_missing_keys() {
    let (params, warnings) = convert(gate, json!({ "reduction": 6.0, "hold": 50.0 }));
    assert_eq!(as_float(&params, "range"), 0.0);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("reverse mode")),
        "{warnings:?}"
    );
    assert!(
        warnings.iter().any(|warning| warning.contains("hold")),
        "{warnings:?}"
    );
}

#[test]
fn both_deesser_mode_labels_are_recognised() {
    let (wide, _) = convert(deesser, json!({ "mode": "Wide" }));
    assert_eq!(as_float(&wide, "mode"), 1.0);
    let (wideband, _) = convert(deesser, json!({ "mode": "Wideband" }));
    assert_eq!(as_float(&wideband, "mode"), 1.0);
    let (split, _) = convert(deesser, json!({ "mode": "Split" }));
    assert_eq!(as_float(&split, "mode"), 0.0);
}

#[test]
fn a_missing_deesser_mode_keeps_the_reference_default() {
    let (params, warnings) = convert(deesser, json!({ "threshold": -18.0 }));
    assert_eq!(as_float(&params, "mode"), 1.0);
    assert!(warnings.is_empty(), "{warnings:?}");
    let (unknown, warnings) = convert(deesser, json!({ "mode": "Narrow" }));
    assert_eq!(as_float(&unknown, "mode"), 1.0);
    assert!(
        warnings.iter().any(|warning| warning.contains("Narrow")),
        "{warnings:?}"
    );
}

#[test]
fn the_autogain_silence_threshold_is_transferred() {
    let (params, _) = convert(
        autogain,
        json!({ "target": -18.0, "silence-threshold": -60.0 }),
    );
    assert_eq!(as_float(&params, "target"), -18.0);
    assert_eq!(as_float(&params, "silence_threshold"), -60.0);
}

#[test]
fn the_expander_knee_is_a_width_and_the_wet_level_is_a_mix() {
    let (params, warnings) = convert(
        expander,
        json!({
            "threshold": -15.0,
            "ratio": 8.0,
            "knee": -6.0,
            "attack": 30.0,
            "release": 150.0,
            "makeup": 3.0,
            "wet": 0.0
        }),
    );
    assert!((as_float(&params, "threshold") + 15.0).abs() < 1.0e-9);
    assert_eq!(as_float(&params, "ratio"), 8.0);
    assert!((as_float(&params, "knee") - 6.0).abs() < 1.0e-9);
    assert_eq!(as_float(&params, "attack"), 30.0);
    assert_eq!(as_float(&params, "release"), 150.0);
    assert_eq!(as_float(&params, "makeup"), 3.0);
    assert!((as_float(&params, "mix") - 100.0).abs() < 0.1);
    assert_eq!(as_float(&params, "detector"), 0.0);
    assert!(
        warnings
            .iter()
            .all(|warning| !warning.contains("expander:")),
        "{warnings:?}"
    );
}

#[test]
fn the_upward_expander_is_reported() {
    let (params, warnings) = convert(expander, json!({ "mode": "Upward" }));
    assert_eq!(as_float(&params, "threshold"), -12.0);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("upward mode")),
        "{warnings:?}"
    );
}

#[test]
fn the_expander_detector_follows_the_sidechain() {
    let (rms, _) = convert(expander, json!({ "sidechain": { "mode": "RMS" } }));
    assert_eq!(as_float(&rms, "detector"), 1.0);
    let (other, warnings) = convert(expander, json!({ "sidechain": { "mode": "SMA" } }));
    assert_eq!(as_float(&other, "detector"), 0.0);
    assert!(
        warnings.iter().any(|warning| warning.contains("SMA")),
        "{warnings:?}"
    );
}
