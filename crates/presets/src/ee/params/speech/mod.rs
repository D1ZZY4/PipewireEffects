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
#[path = "tests/tests.rs"]
mod tests;
