use std::collections::BTreeMap;

use serde_json::Value;

use super::{convert, is_ees};
use crate::model::{Preset, PresetValue};

const SAMPLE: &str = r#"{
      "output": {
        "plugins_order": ["filter#0", "equalizer#0", "maximizer#0"],
        "filter#0": { "type": "High-pass", "frequency": 22.0, "quality": 0.71, "slope": "x2" },
        "equalizer#0": {
          "input-gain": -2.5, "output-gain": 0.0,
          "left": {
            "band0": { "frequency": 70.0, "gain": 1.8, "type": "Low Shelf" },
            "band1": { "frequency": 1000.0, "gain": 0.0, "type": "Bell" },
            "band2": { "frequency": 16000.0, "gain": 0.5, "type": "High Shelf" }
          }
        },
        "maximizer#0": { "threshold": -2.5 }
      }
    }"#;

fn sample() -> (Preset, Vec<String>) {
    let value: Value = serde_json::from_str(SAMPLE).unwrap();
    assert!(is_ees(&value));
    convert(&value, "Imported").unwrap()
}

#[test]
fn supported_effects_are_mapped_in_order() {
    let (preset, warnings) = sample();
    assert_eq!(preset.output.effects.len(), 2);
    assert_eq!(preset.output.effects[0].kind, "filter");
    assert_eq!(preset.output.effects[1].kind, "equalizer");
    assert!(warnings.iter().any(|warning| warning.contains("maximizer")));
}

#[test]
fn the_filter_mode_and_order_are_translated() {
    let (preset, _) = sample();
    let params = &preset.output.effects[0].params;
    assert_eq!(params.get("mode"), Some(&PresetValue::Float(1.0)));
    assert_eq!(params.get("order"), Some(&PresetValue::Float(2.0)));
}

fn gain(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

#[test]
fn equalizer_bands_are_folded_onto_the_fixed_grid() {
    let (preset, _) = sample();
    let params = &preset.output.effects[1].params;
    assert!((gain(params, "band2_gain") - 1.8).abs() < 1.0e-6);
    assert!((gain(params, "band10_gain") - 0.5).abs() < 1.0e-6);
    assert!((gain(params, "output_gain") + 2.5).abs() < 1.0e-6);
}

#[test]
fn a_bypassed_effect_stays_disabled() {
    let value: Value = serde_json::from_str(
            r#"{ "output": { "plugins_order": ["limiter#0"], "limiter#0": { "bypass": true, "threshold": -1.0 } } }"#,
        )
        .unwrap();
    let (preset, _) = convert(&value, "Bypassed").unwrap();
    assert!(!preset.output.effects[0].enabled);
}

#[test]
fn a_document_without_mappable_effects_is_rejected() {
    let value: Value = serde_json::from_str(
        r#"{ "output": { "plugins_order": ["maximizer#0"], "maximizer#0": {} } }"#,
    )
    .unwrap();
    assert!(convert(&value, "Empty").is_err());
}
