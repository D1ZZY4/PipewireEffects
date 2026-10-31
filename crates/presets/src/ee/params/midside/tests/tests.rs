use serde_json::json;

use super::*;

fn convert(value: Value) -> (BTreeMap<String, PresetValue>, Vec<String>) {
    let parameters = value.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = midside_equalizer(&parameters, &mut warnings);
    (params, warnings)
}

fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

#[test]
fn the_mid_and_side_blocks_reach_their_own_bands() {
    let (params, warnings) = convert(json!({
        "mid": { "band0": { "frequency": 1000.0, "gain": 4.0, "q": 1.4, "type": "Bell" } },
        "side": { "band0": { "frequency": 4000.0, "gain": -3.0, "q": 1.4, "type": "Bell" } }
    }));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!((as_float(&params, "mid_band6_gain") - 4.0).abs() < 1.0e-6);
    assert!((as_float(&params, "side_band8_gain") + 3.0).abs() < 1.0e-6);
    assert_eq!(as_float(&params, "mid_band8_gain"), 0.0);
}

#[test]
fn the_output_gain_is_transferred() {
    let (params, _) = convert(json!({ "output-gain": -2.5 }));
    assert!((as_float(&params, "output_gain") + 2.5).abs() < 1.0e-9);
}

#[test]
fn settings_without_an_equivalent_are_reported() {
    let (_, warnings) = convert(json!({
        "mode": "FFT", "decramp": "x2", "balance": 10.0,
        "num-bands": 32, "split-channels": true
    }));
    let reported = warnings.join(" ");
    assert!(reported.contains("balance"), "{reported}");
    assert!(reported.contains("num-bands"), "{reported}");
}

#[test]
fn a_missing_channel_block_leaves_its_bands_flat() {
    let (params, _) = convert(json!({
        "side": { "band0": { "frequency": 250.0, "gain": 6.0, "type": "Bell" } }
    }));
    assert!(!params.contains_key("mid_band4_gain"));
    assert!((as_float(&params, "side_band4_gain") - 6.0).abs() < 1.0e-6);
}

#[test]
fn the_side_bandwidth_is_used_when_the_mid_block_has_none() {
    let (params, _) = convert(json!({
        "mid": { "band0": { "frequency": 1000.0, "gain": 2.0, "type": "Bell" } },
        "side": { "band0": { "frequency": 1000.0, "gain": 3.0, "q": 2.0, "type": "Bell" } }
    }));
    assert!((as_float(&params, "bandwidth") - 2.0).abs() < 1.0e-9);
}
