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

const REVERSE_ENGINEERED: &str = r#"{
      "output": {
        "plugins_order": ["gate#0", "delay#0", "reverb#0", "deesser#0", "stereo_tools#0"],
        "gate#0": {
          "curve-threshold": -30.0, "curve-zone": -6.0, "reduction": -40.0,
          "attack": 10.0, "release": 250.0, "wet": 0.0, "hysteresis": true
        },
        "delay#0": {
          "mode-l": "Time", "mode-r": "Time", "time-l": 120.0, "time-r": 250.0,
          "wet-l": 0.0, "wet-r": 0.0, "dry-l": -80.01, "invert-phase-l": false
        },
        "reverb#0": {
          "room-size": "Large", "decay-time": 1.5, "hf-damp": 5000.0,
          "amount": -12.0, "dry": 0.0, "predelay": 22.0, "diffusion": 0.5
        },
        "deesser#0": { "mode": "Wide", "f1-freq": 5600.0, "threshold": -15.0, "ratio": 1.35 },
        "stereo_tools#0": { "side-level": 0.0, "balance-out": 0.0, "stereo-base": 0.25 }
      }
    }"#;

fn sample() -> (Preset, Vec<String>) {
    let value: Value = serde_json::from_str(SAMPLE).unwrap();
    assert!(is_ees(&value));
    convert(&value, "Imported").unwrap()
}

fn reverse_engineered() -> (Preset, Vec<String>) {
    let value: Value = serde_json::from_str(REVERSE_ENGINEERED).unwrap();
    convert(&value, "Imported").unwrap()
}

fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

fn params_of(preset: &Preset, kind: &str) -> BTreeMap<String, PresetValue> {
    preset
        .output
        .effects
        .iter()
        .find(|entry| entry.kind == kind)
        .map(|entry| entry.params.clone())
        .unwrap_or_default()
}

#[test]
fn supported_effects_are_mapped_in_order() {
    let (preset, warnings) = sample();
    assert_eq!(preset.output.effects.len(), 3);
    assert_eq!(preset.output.effects[0].kind, "filter");
    assert_eq!(preset.output.effects[1].kind, "equalizer");
    assert_eq!(preset.output.effects[2].kind, "maximizer");
    assert!(
        warnings
            .iter()
            .all(|warning| !warning.contains("maximizer")),
        "{warnings:?}"
    );
}

#[test]
fn the_maximizer_threshold_is_transferred() {
    let (preset, _) = sample();
    let params = params_of(&preset, "maximizer");
    assert!((as_float(&params, "threshold") + 2.5).abs() < 1.0e-6);
    assert!((as_float(&params, "release") - 25.0).abs() < 1.0e-6);
}

#[test]
fn the_filter_mode_and_order_are_translated() {
    let (preset, _) = sample();
    let params = &preset.output.effects[0].params;
    assert_eq!(params.get("mode"), Some(&PresetValue::Float(1.0)));
    assert_eq!(params.get("order"), Some(&PresetValue::Float(2.0)));
}

#[test]
fn equalizer_bands_are_folded_onto_the_fixed_grid() {
    let (preset, _) = sample();
    let params = &preset.output.effects[1].params;
    assert!((as_float(params, "band2_gain") - 1.8).abs() < 1.0e-6);
    assert!((as_float(params, "band10_gain") - 0.5).abs() < 1.0e-6);
    assert!((as_float(params, "output_gain") + 2.5).abs() < 1.0e-6);
    assert!((as_float(params, "bandwidth") - 1.414).abs() < 1.0e-6);
}

#[test]
fn the_equalizer_quality_becomes_the_shared_bandwidth() {
    let value: Value = serde_json::from_str(
        r#"{ "output": { "plugins_order": ["equalizer#0"], "equalizer#0": { "left": {
            "band0": { "frequency": 1000.0, "gain": 1.0, "q": 2.5, "type": "Bell" } } } } }"#,
    )
    .unwrap();
    let (preset, _) = convert(&value, "Imported").unwrap();
    assert!((as_float(&preset.output.effects[0].params, "bandwidth") - 2.5).abs() < 1.0e-6);
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
        r#"{ "output": { "plugins_order": ["convolver#0"], "convolver#0": {} } }"#,
    )
    .unwrap();
    assert!(convert(&value, "Empty").is_err());
}

#[test]
fn a_parameter_block_keyed_without_the_instance_suffix_is_found() {
    let value: Value = serde_json::from_str(
        r#"{ "output": { "plugins_order": ["filter#0"],
             "filter": { "type": "High-pass", "frequency": 30.0 } } }"#,
    )
    .unwrap();
    let (preset, warnings) = convert(&value, "Legacy").unwrap();
    assert_eq!(preset.output.effects[0].kind, "filter");
    assert!(
        warnings
            .iter()
            .all(|warning| !warning.contains("parameter block")),
        "{warnings:?}"
    );
}

#[test]
fn a_per_plugin_gain_without_an_equivalent_is_reported() {
    let value: Value = serde_json::from_str(
        r#"{ "output": { "plugins_order": ["filter#0"],
             "filter#0": { "type": "High-pass", "input-gain": -6.0, "output-gain": 3.0 } } }"#,
    )
    .unwrap();
    let (_, warnings) = convert(&value, "Gain").unwrap();
    let reported = warnings.join(" ");
    assert!(reported.contains("input-gain"), "{reported}");
    assert!(reported.contains("output-gain"), "{reported}");
}

#[test]
fn the_gate_uses_the_curve_threshold_and_reduction() {
    let (preset, _) = reverse_engineered();
    let params = params_of(&preset, "gate");
    assert!((as_float(&params, "threshold") + 30.0).abs() < 1.0e-6);
    assert!((as_float(&params, "range") - 40.0).abs() < 1.0e-6);
    assert!((as_float(&params, "attack") - 10.0).abs() < 1.0e-6);
    assert!((as_float(&params, "mix") - 100.0).abs() < 0.1);
}

#[test]
fn the_delay_uses_the_left_and_right_times() {
    let (preset, _) = reverse_engineered();
    let params = params_of(&preset, "delay");
    assert!((as_float(&params, "delay_left") - 120.0).abs() < 1.0e-6);
    assert!((as_float(&params, "delay_right") - 250.0).abs() < 1.0e-6);
    assert!((as_float(&params, "mix") - 100.0).abs() < 0.1);
    assert_eq!(as_float(&params, "feedback"), 0.0);
}

#[test]
fn the_reverb_keeps_its_decay_and_mix() {
    let (preset, _) = reverse_engineered();
    let params = params_of(&preset, "reverb");
    assert!((as_float(&params, "room_size") - 60.0).abs() < 1.0e-6);
    assert!((as_float(&params, "pre_delay") - 22.0).abs() < 1.0e-6);
    assert!((as_float(&params, "wet") + 12.0).abs() < 1.0e-6);
}

#[test]
fn the_wide_deesser_mode_is_not_lost() {
    let (preset, _) = reverse_engineered();
    let params = params_of(&preset, "deesser");
    assert_eq!(as_float(&params, "mode"), 1.0);
    assert!((as_float(&params, "frequency") - 5600.0).abs() < 1.0e-6);
}

#[test]
fn a_unity_side_level_keeps_the_stereo_image() {
    let (preset, _) = reverse_engineered();
    let params = params_of(&preset, "stereo_tools");
    assert!((as_float(&params, "side_level")).abs() < 1.0e-6);
}

#[test]
fn content_that_cannot_be_mapped_is_reported() {
    let (_, warnings) = reverse_engineered();
    let reported = warnings.join(" ");
    assert!(reported.contains("dry-l"), "{warnings:?}");
    assert!(reported.contains("room-size"), "{warnings:?}");
    assert!(reported.contains("stereo-base"), "{warnings:?}");
    assert!(reported.contains("invert-phase-l"), "{warnings:?}");
}

#[test]
fn the_reverb_tone_controls_are_transferred() {
    let value: Value = serde_json::from_str(
        r#"{ "output": { "plugins_order": ["reverb#0"],
             "reverb#0": { "diffusion": 0.25, "bass-cut": 200.0, "treble-cut": 4000.0 } } }"#,
    )
    .unwrap();
    let (preset, warnings) = convert(&value, "Reverb").unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let params = params_of(&preset, "reverb");
    assert!((as_float(&params, "diffusion") - 25.0).abs() < 1.0e-6);
    assert!((as_float(&params, "bass_cut") - 200.0).abs() < 1.0e-6);
    assert!((as_float(&params, "treble_cut") - 4000.0).abs() < 1.0e-6);
}

#[test]
fn the_gate_curve_is_transferred_and_its_hysteresis_is_reported() {
    let (preset, warnings) = reverse_engineered();
    let params = params_of(&preset, "gate");
    assert!((as_float(&params, "curve") - 6.0).abs() < 1.0e-6);
    let reported = warnings.join(" ");
    assert!(reported.contains("hysteresis"), "{warnings:?}");
}

#[test]
fn the_limiter_lookahead_and_stereo_link_are_transferred() {
    let value: Value = serde_json::from_str(
        r#"{ "output": { "plugins_order": ["limiter#0"],
             "limiter#0": { "threshold": -2.0, "lookahead": 8.0, "stereoLink": 40.0 } } }"#,
    )
    .unwrap();
    let (preset, warnings) = convert(&value, "Limiter").unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let params = params_of(&preset, "limiter");
    assert!((as_float(&params, "lookahead") - 8.0).abs() < 1.0e-6);
    assert!((as_float(&params, "stereo_link") - 40.0).abs() < 1.0e-6);
}

const BOTH_SECTIONS: &str = r#"{
      "output": {
        "plugins_order": ["limiter#0"],
        "limiter#0": { "threshold": -1.0 }
      },
      "input": {
        "plugins_order": ["gate#0", "unknown_effect#0"],
        "gate#0": { "curve-threshold": -32.0, "reduction": -30.0 }
      }
    }"#;

#[test]
fn the_input_section_is_converted_as_well() {
    let value: Value = serde_json::from_str(BOTH_SECTIONS).unwrap();
    let (preset, warnings) = convert(&value, "Imported").unwrap();
    assert_eq!(preset.output.effects.len(), 1);
    assert_eq!(preset.input.effects.len(), 1);
    assert_eq!(preset.input.effects[0].kind, "gate");
    assert!(
        warnings.iter().any(|warning| warning.starts_with("input:")),
        "an input warning should name its section: {warnings:?}"
    );
}

#[test]
fn an_input_only_document_is_accepted() {
    let value: Value = serde_json::from_str(
        r#"{"input": {"plugins_order": ["limiter#0"], "limiter#0": {"threshold": -3.0}}}"#,
    )
    .unwrap();
    assert!(is_ees(&value));
    let (preset, _) = convert(&value, "Imported").unwrap();
    assert!(preset.output.effects.is_empty());
    assert_eq!(preset.input.effects.len(), 1);
}

#[test]
fn the_input_section_records_its_bypass_state() {
    let value: Value = serde_json::from_str(
        r#"{"output": {"bypass": true, "plugins_order": ["limiter#0"], "limiter#0": {"threshold": -3.0}}}"#,
    )
    .unwrap();
    let (preset, _) = convert(&value, "Imported").unwrap();
    assert!(preset.output.bypassed);
    assert!(!preset.input.bypassed);
}
