use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{flag, number, put, skipped};

const SPEECH_KEYS: &[&str] = &[
    "bypass",
    "enable-denoise",
    "noise-suppression",
    "enable-agc",
    "vad",
];

pub(super) fn speech_processor(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, SPEECH_KEYS, "speech processor", warnings);
    let vad = parameters.get("vad").and_then(Value::as_object);
    if let Some(vad) = vad {
        let unhandled: Vec<&str> = vad
            .keys()
            .map(String::as_str)
            .filter(|key| *key != "enable")
            .collect();
        if !unhandled.is_empty() {
            warnings.push(tr(
                "speech processor: vad.{0} was left at its default",
                &[&unhandled.join(", vad.")],
            ));
        }
    }
    let mut params = BTreeMap::new();
    params.insert(
        "vad".to_owned(),
        PresetValue::Boolean(vad.is_some_and(|values| flag(values, "enable", false) >= 0.5)),
    );
    params.insert(
        "agc".to_owned(),
        PresetValue::Boolean(flag(parameters, "enable-agc", false) >= 0.5),
    );
    params.insert(
        "denoise".to_owned(),
        PresetValue::Boolean(flag(parameters, "enable-denoise", false) >= 0.5),
    );
    put(
        &mut params,
        "depth",
        number(parameters, "noise-suppression", -30.0)
            .abs()
            .clamp(0.0, 60.0),
    );
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
}
