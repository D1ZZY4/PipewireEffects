use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{flag, number, put, skipped};

const LOUDNESS_KEYS: &[&str] = &[
    "bypass",
    "mode",
    "std",
    "volume",
    "clipping",
    "clippingRange",
];

const STANDARD_COUNT: f64 = 5.0;
const IIR_MODE: f64 = 1.0;

pub(super) fn loudness(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, LOUDNESS_KEYS, "loudness", warnings);
    if number(parameters, "mode", IIR_MODE) != IIR_MODE {
        warnings.push(tr(
            "loudness: the FFT mode has no equivalent here and the IIR contour is used instead",
            &[],
        ));
    }
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "standard",
        number(parameters, "std", 4.0).clamp(0.0, STANDARD_COUNT - 1.0),
    );
    put(
        &mut params,
        "volume",
        number(parameters, "volume", 0.0).clamp(-83.0, 7.0),
    );
    params.insert(
        "clipping".to_owned(),
        PresetValue::Boolean(flag(parameters, "clipping", false) >= 0.5),
    );
    put(
        &mut params,
        "clipping_range",
        number(parameters, "clippingRange", 6.0).clamp(0.0, 24.0),
    );
    params
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
