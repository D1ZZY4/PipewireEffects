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
#[path = "tests/tests.rs"]
mod tests;
