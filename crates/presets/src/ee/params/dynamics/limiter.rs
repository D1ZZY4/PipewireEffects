use std::collections::BTreeMap;

use pwe_core::i18n::t;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::super::{flag, number, put, skipped};

const LIMITER_KEYS: &[&str] = &[
    "bypass",
    "input-gain",
    "threshold",
    "release",
    "lookahead",
    "stereoLink",
    "oversampling",
    "dithering",
    "attack",
    "gainBoost",
    "alr",
];

pub(in super::super) fn limiter(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, LIMITER_KEYS, "limiter", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "limit", number(parameters, "threshold", 0.0));
    put(&mut params, "release", number(parameters, "release", 5.0));
    put(
        &mut params,
        "lookahead",
        number(parameters, "lookahead", 5.0),
    );
    put(
        &mut params,
        "stereo_link",
        number(parameters, "stereoLink", 100.0),
    );
    put(
        &mut params,
        "input_gain",
        number(parameters, "input-gain", 0.0),
    );
    put(&mut params, "attack", number(parameters, "attack", 20.0));
    put(
        &mut params,
        "gain_boost",
        flag(parameters, "gainBoost", true),
    );
    put(&mut params, "release_mode", flag(parameters, "alr", false));
    put(&mut params, "oversampling", oversampling(parameters));
    put(&mut params, "dithering", dithering(parameters, warnings));
    params
}

fn oversampling(parameters: &Map<String, Value>) -> f64 {
    match number(parameters, "oversampling", 0.0) as i64 {
        0 => 0.0,
        1 | 2 | 11 | 12 | 21 | 22 => 1.0,
        _ => 2.0,
    }
}

fn dithering(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    let index = number(parameters, "dithering", 0.0) as i64;
    if index > 0 {
        warnings
            .push(t("limiter: the reference bit depth was mapped to 16-bit dithering").to_owned());
    }
    match index {
        0 => 0.0,
        1 | 2 => 1.0,
        _ => 2.0,
    }
}
