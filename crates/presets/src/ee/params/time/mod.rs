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
    "room-size",
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
    put(
        &mut params,
        "character",
        room_character(parameters, warnings),
    );
    params
}

fn room_character(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    match text(parameters, "room-size") {
        "" | "Large" => 2.0,
        "Small" => 0.0,
        "Medium" => 1.0,
        "Tunnel-like" => 3.0,
        "Large/smooth" => 4.0,
        "Experimental" => 5.0,
        other => {
            warnings.push(tr(
                "reverb: the room size '{0}' has no equivalent and stays Large",
                &[other],
            ));
            2.0
        }
    }
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
#[path = "tests/tests.rs"]
mod tests;
