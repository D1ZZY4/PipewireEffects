use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{mix_percent, number, put, skipped};

const COMPRESSOR_KEYS: &[&str] = &[
    "bypass",
    "threshold",
    "ratio",
    "knee",
    "attack",
    "release",
    "makeup",
    "wet",
    "sidechain",
];

const LIMITER_KEYS: &[&str] = &["bypass", "input-gain", "threshold", "release"];

const GATE_KEYS: &[&str] = &[
    "bypass",
    "curve-threshold",
    "reduction",
    "attack",
    "release",
    "wet",
];

const DEESSER_KEYS: &[&str] = &["bypass", "f1-freq", "threshold", "ratio", "mode"];

const AUTOGAIN_KEYS: &[&str] = &["bypass", "target", "silence-threshold"];

const DETECTOR_KEYS: &[&str] = &["mode"];

pub(super) fn compressor(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, COMPRESSOR_KEYS, "compressor", warnings);
    skipped_sidechain(parameters, "compressor", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "threshold",
        number(parameters, "threshold", -12.0),
    );
    put(&mut params, "ratio", number(parameters, "ratio", 4.0));
    put(&mut params, "knee", knee(parameters));
    put(&mut params, "attack", number(parameters, "attack", 20.0));
    put(&mut params, "release", number(parameters, "release", 100.0));
    put(&mut params, "makeup", number(parameters, "makeup", 0.0));
    put(&mut params, "mix", mix_percent(parameters, "wet", 0.0));
    put(&mut params, "detector", detector(parameters, warnings));
    params
}

pub(super) fn limiter(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, LIMITER_KEYS, "limiter", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "limit", number(parameters, "threshold", 0.0));
    put(&mut params, "release", number(parameters, "release", 5.0));
    put(
        &mut params,
        "input_gain",
        number(parameters, "input-gain", 0.0),
    );
    params
}

pub(super) fn gate(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, GATE_KEYS, "gate", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "threshold",
        number(parameters, "curve-threshold", -24.0),
    );
    put(&mut params, "range", range(parameters, warnings));
    put(&mut params, "attack", number(parameters, "attack", 20.0));
    put(&mut params, "release", number(parameters, "release", 100.0));
    put(&mut params, "mix", mix_percent(parameters, "wet", 0.0));
    params
}

pub(super) fn deesser(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, DEESSER_KEYS, "deesser", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "frequency",
        number(parameters, "f1-freq", 6_000.0),
    );
    put(
        &mut params,
        "threshold",
        number(parameters, "threshold", -18.0),
    );
    put(&mut params, "ratio", number(parameters, "ratio", 3.0));
    put(&mut params, "mode", mode(parameters, warnings));
    params
}

pub(super) fn autogain(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, AUTOGAIN_KEYS, "autogain", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "target", number(parameters, "target", -23.0));
    put(
        &mut params,
        "silence_threshold",
        number(parameters, "silence-threshold", -70.0),
    );
    params
}

fn knee(parameters: &Map<String, Value>) -> f64 {
    -number(parameters, "knee", -6.0).min(0.0)
}

fn range(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    let reduction = number(parameters, "reduction", -24.0);
    if reduction > 0.0 {
        warnings.push("gate: the reverse mode has no equivalent here".to_owned());
        0.0
    } else {
        -reduction
    }
}

fn mode(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    match super::text(parameters, "mode") {
        "" | "Wide" | "Wideband" => 1.0,
        "Split" => 0.0,
        other => {
            warnings.push(format!(
                "deesser: the mode '{other}' is unknown and was mapped to wideband"
            ));
            1.0
        }
    }
}

fn detector(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    let mode = parameters
        .get("sidechain")
        .and_then(|sidechain| sidechain.get("mode"))
        .and_then(Value::as_str);
    match mode {
        None | Some("Peak") => 0.0,
        Some("RMS") => 1.0,
        Some(other) => {
            warnings.push(format!(
                "compressor: the sidechain mode '{other}' has no equivalent here and was mapped to peak"
            ));
            0.0
        }
    }
}

fn skipped_sidechain(parameters: &Map<String, Value>, effect: &str, warnings: &mut Vec<String>) {
    let Some(sidechain) = parameters.get("sidechain").and_then(Value::as_object) else {
        return;
    };
    let unhandled: Vec<&str> = sidechain
        .keys()
        .map(String::as_str)
        .filter(|key| !DETECTOR_KEYS.contains(key))
        .collect();
    if !unhandled.is_empty() {
        warnings.push(format!(
            "{effect}: sidechain.{} was left at its default",
            unhandled.join(", sidechain.")
        ));
    }
}

#[cfg(test)]
mod tests;
