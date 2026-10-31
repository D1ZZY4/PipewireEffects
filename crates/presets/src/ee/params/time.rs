use std::collections::BTreeMap;

use pwe_core::i18n::{t, tr};
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{level_percent, number, put, skipped, text};

const DELAY_KEYS: &[&str] = &[
    "bypass", "time-l", "time-r", "mode-l", "mode-r", "wet-l", "wet-r", "wet", "feedback",
];

const REVERB_KEYS: &[&str] = &[
    "bypass",
    "predelay",
    "decay-time",
    "hf-damp",
    "diffusion",
    "amount",
    "dry",
    "bass-cut",
    "treble-cut",
];

const WET_KEYS: [&str; 3] = ["wet-l", "wet-r", "wet"];
const DEFAULT_DELAY_MS: f64 = 0.0;
const DECAY_TO_ROOM: f64 = 40.0;
const DAMPING_HERTZ_TO_PERCENT: f64 = 200.0;
const WET_TOLERANCE_DB: f64 = 0.01;

pub(super) fn delay(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, DELAY_KEYS, "delay", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "delay_left",
        delay_time(parameters, "time-l", "mode-l", warnings),
    );
    put(
        &mut params,
        "delay_right",
        delay_time(parameters, "time-r", "mode-r", warnings),
    );
    put(&mut params, "feedback", number(parameters, "feedback", 0.0));
    put(&mut params, "mix", wet_mix(parameters, warnings));
    params
}

pub(super) fn reverb(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, REVERB_KEYS, "reverb", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "pre_delay",
        number(parameters, "predelay", 0.0),
    );
    put(
        &mut params,
        "room_size",
        (number(parameters, "decay-time", 1.5) * DECAY_TO_ROOM).min(100.0),
    );
    put(
        &mut params,
        "damping",
        (number(parameters, "hf-damp", 5_000.0) / DAMPING_HERTZ_TO_PERCENT).min(100.0),
    );
    put(&mut params, "wet", number(parameters, "amount", -12.0));
    put(&mut params, "dry", number(parameters, "dry", 0.0));
    put(
        &mut params,
        "diffusion",
        (number(parameters, "diffusion", 0.5) * 100.0).clamp(0.0, 100.0),
    );
    put(
        &mut params,
        "bass_cut",
        number(parameters, "bass-cut", 300.0),
    );
    put(
        &mut params,
        "treble_cut",
        number(parameters, "treble-cut", 5_000.0),
    );
    params
}

fn delay_time(
    parameters: &Map<String, Value>,
    time_key: &str,
    mode_key: &str,
    warnings: &mut Vec<String>,
) -> f64 {
    match text(parameters, mode_key) {
        "" | "Time" => number(parameters, time_key, DEFAULT_DELAY_MS),
        other => {
            warnings.push(tr(
                "delay: the '{0}' mode for '{1}' has no equivalent here",
                &[other, mode_key],
            ));
            DEFAULT_DELAY_MS
        }
    }
}

fn wet_mix(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    let left = parameters.get("wet-l").and_then(Value::as_f64);
    let right = parameters.get("wet-r").and_then(Value::as_f64);
    if let (Some(left), Some(right)) = (left, right)
        && (left - right).abs() > WET_TOLERANCE_DB
    {
        warnings.push(
            t("delay: the left and right wet levels differ and were reduced to a single mix")
                .to_owned(),
        );
    }
    let mut loudest: Option<f64> = None;
    for key in WET_KEYS {
        if let Some(value) = parameters.get(key).and_then(Value::as_f64) {
            loudest = Some(loudest.map_or(value, |current: f64| current.max(value)));
        }
    }
    level_percent(loudest.unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
    fn the_delay_times_are_ms_values() {
        let (params, warnings) = convert(
            delay,
            json!({ "time-l": 120.0, "time-r": 250.0, "mode-l": "Time", "mode-r": "Time" }),
        );
        assert_eq!(as_float(&params, "delay_left"), 120.0);
        assert_eq!(as_float(&params, "delay_right"), 250.0);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_delay_without_a_feedback_loop_gets_no_feedback() {
        let (params, _) = convert(delay, json!({ "time-l": 10.0 }));
        assert_eq!(as_float(&params, "feedback"), 0.0);
    }

    #[test]
    fn a_non_time_mode_is_reported_and_keeps_the_default() {
        let (params, warnings) = convert(delay, json!({ "mode-l": "Samples", "time-l": 0.0 }));
        assert_eq!(as_float(&params, "delay_left"), DEFAULT_DELAY_MS);
        assert!(
            warnings.iter().any(|warning| warning.contains("Samples")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_missing_delay_time_matches_the_reference_default() {
        let (params, warnings) = convert(delay, json!({ "mode-l": "Time", "mode-r": "Time" }));
        assert_eq!(as_float(&params, "delay_left"), 0.0);
        assert_eq!(as_float(&params, "delay_right"), 0.0);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn the_wet_level_becomes_the_mix() {
        let (loud, _) = convert(delay, json!({ "wet-l": 0.0 }));
        assert!((as_float(&loud, "mix") - 100.0).abs() < 0.1);
        let (silent, _) = convert(delay, json!({ "wet-l": -80.01, "wet-r": -80.01 }));
        assert_eq!(as_float(&silent, "mix"), 0.0);
        let (half, _) = convert(delay, json!({ "wet-l": -6.0 }));
        assert!((as_float(&half, "mix") - 50.1).abs() < 0.6);
        let (absent, _) = convert(delay, json!({ "time-l": 50.0 }));
        assert!((as_float(&absent, "mix") - 100.0).abs() < 0.1);
    }

    #[test]
    fn an_asymmetric_wet_pair_is_reported() {
        let (balanced, warnings) = convert(delay, json!({ "wet-l": -3.0, "wet-r": -3.0 }));
        assert!((as_float(&balanced, "mix") - 70.8).abs() < 0.5);
        assert!(
            warnings.iter().all(|warning| !warning.contains("wet")),
            "{warnings:?}"
        );
        let (asymmetric, warnings) = convert(delay, json!({ "wet-l": 0.0, "wet-r": -80.01 }));
        assert!((as_float(&asymmetric, "mix") - 100.0).abs() < 0.1);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("single mix")),
            "{warnings:?}"
        );
    }

    #[test]
    fn the_reverb_decay_maps_onto_the_room_size() {
        let (params, _) = convert(
            reverb,
            json!({ "decay-time": 1.5, "hf-damp": 5000.0, "predelay": 20.0 }),
        );
        assert!((as_float(&params, "room_size") - 60.0).abs() < 1.0e-9);
        assert!((as_float(&params, "damping") - 25.0).abs() < 1.0e-9);
        assert_eq!(as_float(&params, "pre_delay"), 20.0);
    }

    #[test]
    fn a_very_long_decay_saturates_the_room_size() {
        let (params, _) = convert(reverb, json!({ "decay-time": 15.0 }));
        assert_eq!(as_float(&params, "room_size"), 100.0);
    }

    #[test]
    fn the_reverb_mix_is_a_decibel_pair() {
        let (params, _) = convert(reverb, json!({ "amount": -6.0, "dry": -3.0 }));
        assert_eq!(as_float(&params, "wet"), -6.0);
        assert_eq!(as_float(&params, "dry"), -3.0);
    }

    #[test]
    fn the_reference_room_character_is_reported() {
        let (_, warnings) = convert(reverb, json!({ "room-size": "Large" }));
        assert!(
            warnings.iter().any(|warning| warning.contains("room-size")),
            "{warnings:?}"
        );
    }
}
