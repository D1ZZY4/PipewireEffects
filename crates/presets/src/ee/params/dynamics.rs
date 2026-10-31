use std::collections::BTreeMap;

use pwe_core::i18n::{t, tr};
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{mix_percent, number, put, skipped, text};

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
    "hpf-mode",
    "hpf-frequency",
    "lpf-mode",
    "lpf-frequency",
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

const EXPANDER_KEYS: &[&str] = &[
    "bypass",
    "wet",
    "mode",
    "attack",
    "release",
    "threshold",
    "ratio",
    "knee",
    "makeup",
    "hpf-mode",
    "hpf-frequency",
    "lpf-mode",
    "lpf-frequency",
];

const DETECTOR_KEYS: &[&str] = &["mode"];

const MAXIMIZER_KEYS: &[&str] = &["bypass", "threshold", "release"];

pub(super) fn maximizer(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, MAXIMIZER_KEYS, "maximizer", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "threshold",
        number(parameters, "threshold", 0.0),
    );
    put(&mut params, "release", number(parameters, "release", 25.0));
    params
}

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
    put(
        &mut params,
        "detector",
        detector(parameters, "compressor", warnings),
    );
    put_sidechain(parameters, &mut params, warnings);
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

pub(super) fn expander(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, EXPANDER_KEYS, "expander", warnings);
    skipped_sidechain(parameters, "expander", warnings);
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
    put(
        &mut params,
        "detector",
        detector(parameters, "expander", warnings),
    );
    expander_mode(parameters, warnings);
    put_sidechain(parameters, &mut params, warnings);
    params
}

fn put_sidechain(
    parameters: &Map<String, Value>,
    params: &mut BTreeMap<String, PresetValue>,
    warnings: &mut Vec<String>,
) {
    put(
        params,
        "hpf_mode",
        sidechain_mode(parameters, "hpf-mode", warnings),
    );
    put(
        params,
        "hpf_frequency",
        number(parameters, "hpf-frequency", 10.0),
    );
    put(
        params,
        "lpf_mode",
        sidechain_mode(parameters, "lpf-mode", warnings),
    );
    put(
        params,
        "lpf_frequency",
        number(parameters, "lpf-frequency", 20_000.0),
    );
}

fn sidechain_mode(parameters: &Map<String, Value>, key: &str, warnings: &mut Vec<String>) -> f64 {
    match text(parameters, key) {
        "" | "Off" => 0.0,
        "12 dB/oct" => 1.0,
        "24 dB/oct" => 2.0,
        "36 dB/oct" => 3.0,
        other => {
            warnings.push(tr(
                "the sidechain filter slope '{0}' has no equivalent here and was turned off",
                &[other],
            ));
            0.0
        }
    }
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
        warnings.push(t("gate: the reverse mode has no equivalent here").to_owned());
        0.0
    } else {
        -reduction
    }
}

fn mode(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    match text(parameters, "mode") {
        "" | "Wide" | "Wideband" => 1.0,
        "Split" => 0.0,
        other => {
            warnings.push(tr(
                "deesser: the mode '{0}' is unknown and was mapped to wideband",
                &[other],
            ));
            1.0
        }
    }
}

fn detector(parameters: &Map<String, Value>, effect: &str, warnings: &mut Vec<String>) -> f64 {
    let mode = parameters
        .get("sidechain")
        .and_then(|sidechain| sidechain.get("mode"))
        .and_then(Value::as_str);
    match mode {
        None | Some("Peak") => 0.0,
        Some("RMS") => 1.0,
        Some(other) => {
            warnings.push(tr(
                "{0}: the sidechain mode '{1}' has no equivalent here and was mapped to peak",
                &[effect, other],
            ));
            0.0
        }
    }
}

fn expander_mode(parameters: &Map<String, Value>, warnings: &mut Vec<String>) {
    match text(parameters, "mode") {
        "" | "Downward" => {}
        "Upward" => warnings.push(
            t("expander: the upward mode has no equivalent here and the downward curve was loaded")
                .to_owned(),
        ),
        other => warnings.push(tr(
            "expander: the mode '{0}' is unknown and the downward curve was loaded",
            &[other],
        )),
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
        warnings.push(tr(
            "{0}: sidechain.{1} was left at its default",
            &[effect, &unhandled.join(", sidechain.")],
        ));
    }
}

#[cfg(test)]
mod tests;
