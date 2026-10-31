use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::super::{number, put, text};

const DETECTOR_KEYS: &[&str] = &["mode"];

pub(super) fn put_sidechain(
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

pub(super) fn skipped_sidechain(
    parameters: &Map<String, Value>,
    effect: &str,
    warnings: &mut Vec<String>,
) {
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
