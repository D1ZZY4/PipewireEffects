use std::collections::BTreeMap;

use pwe_core::i18n::{t, tr};
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::equalizer::{DEFAULT_BANDWIDTH, fold, gain_id};
use super::{flag, level_percent, number, put, skipped, skipped_with_bands, text};

const FILTER_KEYS: &[&str] = &["bypass", "type", "frequency", "quality", "gain", "slope"];

const EQUALIZER_KEYS: &[&str] = &["bypass", "input-gain", "output-gain", "left", "right"];

const BASS_ENHANCER_KEYS: &[&str] = &["bypass", "output-gain", "scope", "amount"];

const CRYSTALIZER_KEYS: &[&str] = &["bypass", "output-gain"];

const STEREO_TOOLS_KEYS: &[&str] = &[
    "bypass",
    "output-gain",
    "balance-in",
    "balance-out",
    "softclip",
    "mutel",
    "muter",
    "phasel",
    "phaser",
    "mode",
    "side-level",
    "side-balance",
    "middle-level",
    "middle-panorama",
    "delay",
    "dry",
    "wet",
];

pub(super) fn filter(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, FILTER_KEYS, "filter", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "mode", filter_mode(parameters, warnings));
    put(
        &mut params,
        "frequency",
        number(parameters, "frequency", 2_000.0),
    );
    put(&mut params, "q", filter_quality(parameters));
    put(&mut params, "gain", number(parameters, "gain", 0.0));
    put(&mut params, "order", filter_order(parameters, warnings));
    params
}

pub(super) fn equalizer(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped_with_bands(parameters, EQUALIZER_KEYS, "equalizer", warnings);
    let folded = fold(parameters, "equalizer", warnings);
    let mut params = BTreeMap::new();
    for (band, gain) in folded.gains.iter().enumerate() {
        put(&mut params, gain_id(band), f64::from(*gain));
    }
    put(
        &mut params,
        "bandwidth",
        folded.bandwidth.unwrap_or(DEFAULT_BANDWIDTH),
    );
    let gain = number(parameters, "input-gain", 0.0) + number(parameters, "output-gain", 0.0);
    put(&mut params, "output_gain", gain);
    params
}

pub(super) fn bass_enhancer(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, BASS_ENHANCER_KEYS, "bass_enhancer", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "frequency", number(parameters, "scope", 100.0));
    put(
        &mut params,
        "amount",
        level_percent(number(parameters, "amount", 0.0)),
    );
    put(&mut params, "gain", number(parameters, "output-gain", 0.0));
    params
}

pub(super) fn crystalizer(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, CRYSTALIZER_KEYS, "crystalizer", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "gain", number(parameters, "output-gain", 0.0));
    params
}

pub(super) fn stereo_tools(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, STEREO_TOOLS_KEYS, "stereo_tools", warnings);
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "input_gain",
        number(parameters, "input-gain", 0.0),
    );
    put(&mut params, "mode", stereo_mode(parameters, warnings));
    put(
        &mut params,
        "balance_in",
        number(parameters, "balance-in", 0.0) * 100.0,
    );
    put(
        &mut params,
        "balance_out",
        number(parameters, "balance-out", 0.0) * 100.0,
    );
    put(&mut params, "phase_left", flag(parameters, "phasel", false));
    put(
        &mut params,
        "phase_right",
        flag(parameters, "phaser", false),
    );
    put(&mut params, "mute_left", flag(parameters, "mutel", false));
    put(&mut params, "mute_right", flag(parameters, "muter", false));
    put(&mut params, "softclip", flag(parameters, "softclip", false));
    put(
        &mut params,
        "middle_level",
        number(parameters, "middle-level", 0.0),
    );
    put(
        &mut params,
        "side_level",
        number(parameters, "side-level", 0.0),
    );
    put(
        &mut params,
        "middle_panorama",
        number(parameters, "middle-panorama", 0.0) * 100.0,
    );
    put(
        &mut params,
        "side_balance",
        number(parameters, "side-balance", 0.0) * 100.0,
    );
    put(
        &mut params,
        "stereo_base",
        number(parameters, "delay", 0.0).max(0.0),
    );
    params
}

fn stereo_mode(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    let label = text(parameters, "mode");
    let head = label.split('(').next().unwrap_or("").trim();
    match head {
        "" | "LR > LR" => 0.0,
        "LR > MS" => 1.0,
        "MS > LR" => 2.0,
        "LR > LL" => 3.0,
        "LR > RR" => 4.0,
        "LR > L+R" => 5.0,
        "LR > RL" => 6.0,
        other => {
            warnings.push(tr(
                "stereo tools: the mode '{0}' has no equivalent here and was mapped to stereo",
                &[other],
            ));
            0.0
        }
    }
}

fn filter_mode(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    match text(parameters, "type") {
        "" | "Low-pass" => 0.0,
        "High-pass" => 1.0,
        "Band-pass" => 2.0,
        "Band-reject" | "Notch" => 3.0,
        "All-pass" => 4.0,
        "Bell" | "Resonance" => 5.0,
        "Low-shelf" | "Low Shelf" => 6.0,
        "High-shelf" | "High Shelf" => 7.0,
        "Ladder-pass" => {
            warnings.push(
                t("filter: the reference 'Ladder-pass' type is a resonant ladder low pass and was mapped to a plain low pass")
                    .to_owned(),
            );
            0.0
        }
        "Ladder-rejection" => {
            warnings.push(
                t("filter: the reference 'Ladder-rejection' type is a resonant ladder band rejection and was mapped to a notch")
                    .to_owned(),
            );
            3.0
        }
        other => {
            warnings.push(tr(
                "filter: the type '{0}' has no equivalent here and was mapped to low pass",
                &[other],
            ));
            0.0
        }
    }
}

fn filter_quality(parameters: &Map<String, Value>) -> f64 {
    let quality = number(parameters, "quality", 0.0);
    if quality > 0.0 {
        quality
    } else {
        std::f64::consts::FRAC_1_SQRT_2
    }
}

fn filter_order(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    match text(parameters, "slope") {
        "" | "x1" => 1.0,
        "x2" => 2.0,
        "x3" => 3.0,
        "x4" => 4.0,
        other => {
            warnings.push(tr(
                "filter: the slope '{0}' was reduced to order 4",
                &[other],
            ));
            4.0
        }
    }
}

#[cfg(test)]
mod tests;
