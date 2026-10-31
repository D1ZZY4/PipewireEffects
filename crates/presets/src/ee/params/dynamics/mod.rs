use std::collections::BTreeMap;

use pwe_core::i18n::{t, tr};
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{flag, mix_percent, number, put, skipped, text};

mod sidechain;

use sidechain::{put_sidechain, skipped_sidechain};

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

const GATE_KEYS: &[&str] = &[
    "bypass",
    "curve-threshold",
    "curve-zone",
    "hysteresis-threshold",
    "hysteresis-zone",
    "reduction",
    "attack",
    "release",
    "wet",
    "hpf-mode",
    "hpf-frequency",
    "lpf-mode",
    "lpf-frequency",
    "sidechain",
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

const MAXIMIZER_KEYS: &[&str] = &[
    "bypass",
    "input-gain",
    "output-gain",
    "threshold",
    "release",
];

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
    put(
        &mut params,
        "input_gain",
        number(parameters, "input-gain", 0.0),
    );
    put(
        &mut params,
        "output_gain",
        number(parameters, "output-gain", 0.0),
    );
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
    put(
        &mut params,
        "oversampling",
        limiter_oversampling(parameters),
    );
    put(
        &mut params,
        "dithering",
        limiter_dithering(parameters, warnings),
    );
    params
}

fn limiter_oversampling(parameters: &Map<String, Value>) -> f64 {
    match number(parameters, "oversampling", 0.0) as i64 {
        0 => 0.0,
        1 | 2 | 11 | 12 | 21 | 22 => 1.0,
        _ => 2.0,
    }
}

fn limiter_dithering(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
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

pub(super) fn gate(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, GATE_KEYS, "gate", warnings);
    skipped_sidechain(parameters, "gate", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "threshold",
        number(parameters, "curve-threshold", -24.0),
    );
    put(&mut params, "range", range(parameters, warnings));
    put(&mut params, "attack", number(parameters, "attack", 20.0));
    put(&mut params, "release", number(parameters, "release", 100.0));
    put(
        &mut params,
        "curve",
        number(parameters, "curve-zone", -6.0).abs(),
    );
    put(&mut params, "mix", mix_percent(parameters, "wet", 0.0));
    put_sidechain(parameters, &mut params, warnings);
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

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
