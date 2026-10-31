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
#[path = "tests/tests.rs"]
mod tests;
