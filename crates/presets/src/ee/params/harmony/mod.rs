use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{level_percent, mix_percent, number, put, skipped};

const AUTOTUNE_KEYS: &[&str] = &["bypass", "correction", "tuning"];

const HARMONIZER_KEYS: &[&str] = &[
    "bypass",
    "dry-wet",
    "interval-left",
    "interval-right",
    "gain-left",
    "gain-right",
];

pub(super) fn autotune(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, AUTOTUNE_KEYS, "autotune", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "speed",
        number(parameters, "correction", 30.0)
            .abs()
            .clamp(5.0, 200.0),
    );
    let tuning = number(parameters, "tuning", 440.0);
    if !(420.0..=460.0).contains(&tuning) {
        warnings.push(tr(
            "autotune: a reference pitch of {0} Hz is outside the range here and was clamped",
            &[&format!("{tuning:.1}")],
        ));
    }
    put(&mut params, "reference", tuning.clamp(420.0, 460.0));
    params
}

pub(super) fn harmonizer(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, HARMONIZER_KEYS, "harmonizer", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "interval1",
        number(parameters, "interval-left", 7.0).clamp(-24.0, 24.0),
    );
    put(
        &mut params,
        "interval2",
        number(parameters, "interval-right", 12.0).clamp(-24.0, 24.0),
    );
    put(
        &mut params,
        "level1",
        level_percent(number(parameters, "gain-left", 0.0)),
    );
    put(
        &mut params,
        "level2",
        level_percent(number(parameters, "gain-right", 0.0)),
    );
    put(&mut params, "mix", mix_percent(parameters, "dry-wet", 0.0));
    params
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
