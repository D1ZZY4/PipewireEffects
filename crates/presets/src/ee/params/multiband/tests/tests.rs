use serde_json::json;

use super::*;

fn parameters(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[test]
fn the_split_frequencies_and_band_values_transfer() {
    let parameters = parameters(json!({
        "compressor-mode": "Classic",
        "band0": { "compressor-enable": true, "attack-threshold": -30.0, "ratio": 4.0 },
        "band1": { "enable-band": true, "split-frequency": 250.0, "attack-threshold": -24.0, "ratio": 3.0 },
        "band2": { "enable-band": true, "split-frequency": 2500.0, "attack-threshold": -18.0 }
    }));
    let mut warnings = Vec::new();
    let params = multiband_compressor(&parameters, &mut warnings);
    assert_eq!(params.get("bands"), Some(&PresetValue::Float(3.0)));
    assert_eq!(params.get("split1"), Some(&PresetValue::Float(250.0)));
    assert_eq!(params.get("split2"), Some(&PresetValue::Float(2500.0)));
    assert_eq!(
        params.get("band2_threshold"),
        Some(&PresetValue::Float(-24.0))
    );
}

#[test]
fn all_eight_bands_transfer() {
    let mut value = Map::new();
    for index in 0..8 {
        value.insert(
            format!("band{index}"),
            json!({ "enable-band": true, "split-frequency": 100.0 * (index as f64 + 1.0) }),
        );
    }
    let mut warnings = Vec::new();
    let params = multiband_compressor(&parameters(Value::Object(value)), &mut warnings);
    assert_eq!(params.get("bands"), Some(&PresetValue::Float(8.0)));
    assert_eq!(params.get("split7"), Some(&PresetValue::Float(800.0)));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_gate_reduction_becomes_a_positive_range() {
    let parameters = parameters(json!({
        "band0": { "gate-enable": true, "curve-threshold": -30.0, "reduction": -24.0 }
    }));
    let mut warnings = Vec::new();
    let params = multiband_gate(&parameters, &mut warnings);
    assert_eq!(params.get("band1_range"), Some(&PresetValue::Float(24.0)));
    assert_eq!(
        params.get("band1_threshold"),
        Some(&PresetValue::Float(-30.0))
    );
}
