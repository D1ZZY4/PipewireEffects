use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{number, put, skipped};

const BASS_LOUDNESS_KEYS: &[&str] = &["bypass", "loudness", "output", "link"];

pub(super) fn bass_loudness(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, BASS_LOUDNESS_KEYS, "bass loudness", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "loudness",
        number(parameters, "loudness", -3.1),
    );
    put(&mut params, "output", number(parameters, "output", -6.0));
    put(&mut params, "link", number(parameters, "link", -9.1));
    params
}

#[cfg(test)]
mod tests {
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
}
