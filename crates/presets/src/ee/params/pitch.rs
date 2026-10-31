use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{mix_percent, number, put, skipped};

const PITCH_KEYS: &[&str] = &[
    "bypass",
    "inputGain",
    "outputGain",
    "wet",
    "cents",
    "semitones",
    "octaves",
];

const MAX_SEMITONES: f64 = 24.0;

pub(super) fn pitch_shift(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, PITCH_KEYS, "pitch", warnings);
    let shift = number(parameters, "semitones", 0.0) + number(parameters, "octaves", 0.0) * 12.0;
    if shift.abs() > MAX_SEMITONES {
        warnings.push(tr(
            "pitch: {0} semitones is wider than the range here and was clamped",
            &[&format!("{shift:.1}")],
        ));
    }
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "semitones",
        shift.clamp(-MAX_SEMITONES, MAX_SEMITONES),
    );
    put(&mut params, "cents", number(parameters, "cents", 0.0));
    put(&mut params, "mix", mix_percent(parameters, "wet", 0.0));
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
        let params = pitch_shift(&parameters, &mut warnings);
        (params, warnings)
    }

    #[test]
    fn semitones_octaves_and_cents_are_summed() {
        let (params, warnings) = convert(json!({
            "semitones": 2.0, "octaves": 1.0, "cents": 25.0, "wet": 0.0
        }));
        assert!((as_float(&params, "semitones") - 14.0).abs() < 1.0e-9);
        assert!((as_float(&params, "cents") - 25.0).abs() < 1.0e-9);
        assert!((as_float(&params, "mix") - 100.0).abs() < 0.1);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_shift_wider_than_the_range_is_reported_and_clamped() {
        let (params, warnings) = convert(json!({ "semitones": 12.0, "octaves": 3.0 }));
        assert!((as_float(&params, "semitones") - MAX_SEMITONES).abs() < 1.0e-9);
        assert!(
            warnings.iter().any(|warning| warning.contains("48.0")),
            "{warnings:?}"
        );
    }

    #[test]
    fn the_reference_default_is_a_transparent_shift() {
        let (params, warnings) = convert(json!({ "wet": 0.0, "dry": -100.0 }));
        assert_eq!(as_float(&params, "semitones"), 0.0);
        assert!((as_float(&params, "mix") - 100.0).abs() < 0.1);
        assert!(
            warnings.iter().any(|warning| warning.contains("dry")),
            "{warnings:?}"
        );
    }

    #[test]
    fn the_soundtouch_settings_have_no_equivalent_and_are_reported() {
        let (_, warnings) = convert(json!({
            "semitones": 1.0, "quickSeek": true, "antiAlias": false,
            "sequenceLength": 40, "seekWindow": 15, "overlapLength": 8,
            "tempoDifference": 0.0, "rateDifference": 0.0
        }));
        let reported = warnings.join(" ");
        assert!(reported.contains("quickSeek"), "{reported}");
        assert!(reported.contains("tempoDifference"), "{reported}");
    }

    #[test]
    fn a_quiet_wet_level_becomes_a_partial_mix() {
        let (params, _) = convert(json!({ "wet": -6.0 }));
        assert!((as_float(&params, "mix") - 50.1).abs() < 0.6);
    }
}
