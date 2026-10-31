use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{flag, mix_percent, number, put, skipped, text};

const CRUSHER_KEYS: &[&str] = &[
    "bypass",
    "mode",
    "bit-reduction",
    "morph",
    "anti-aliasing",
    "sample-reduction",
    "lfo-active",
    "lfo-range",
    "lfo-rate",
    "dc",
];

const CROSSFEED_KEYS: &[&str] = &["bypass", "fcut", "feed"];

const CROSSTALK_KEYS: &[&str] = &["bypass", "delay-us", "decay-db", "phantom-center-only"];

const EXCITER_KEYS: &[&str] = &[
    "bypass",
    "amount",
    "harmonics",
    "scope",
    "ceil",
    "blend",
    "ceil-active",
];

pub(super) fn crusher(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, CRUSHER_KEYS, "crusher", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "mode", crusher_mode(parameters, warnings));
    put(
        &mut params,
        "bits",
        number(parameters, "bit-reduction", 4.0),
    );
    put(&mut params, "mix", number(parameters, "morph", 0.5) * 100.0);
    put(
        &mut params,
        "anti_aliasing",
        number(parameters, "anti-aliasing", 0.5) * 100.0,
    );
    put(
        &mut params,
        "samples",
        number(parameters, "sample-reduction", 1.0),
    );
    put(
        &mut params,
        "lfo_active",
        flag(parameters, "lfo-active", false),
    );
    put(
        &mut params,
        "lfo_depth",
        number(parameters, "lfo-range", 20.0),
    );
    put(&mut params, "lfo_rate", number(parameters, "lfo-rate", 0.3));
    put(&mut params, "dc", number(parameters, "dc", 0.0));
    params
}

fn crusher_mode(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    match text(parameters, "mode") {
        "" | "Linear" => 0.0,
        "Logarithmic" => 1.0,
        other => {
            warnings.push(tr(
                "crusher: the mode '{0}' has no equivalent here and was mapped to linear",
                &[other],
            ));
            0.0
        }
    }
}

pub(super) fn crossfeed(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, CROSSFEED_KEYS, "crossfeed", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "cutoff", number(parameters, "fcut", 700.0));
    put(&mut params, "feed", number(parameters, "feed", 4.5));
    params
}

pub(super) fn crosstalk(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, CROSSTALK_KEYS, "crosstalk", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "delay", number(parameters, "delay-us", 313.0));
    put(&mut params, "decay", number(parameters, "decay-db", -2.0));
    put(
        &mut params,
        "phantom_center",
        flag(parameters, "phantom-center-only", false),
    );
    params
}

pub(super) fn exciter(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, EXCITER_KEYS, "exciter", warnings);
    let mut params = BTreeMap::new();
    put(&mut params, "amount", number(parameters, "amount", 0.0));
    put(
        &mut params,
        "harmonics",
        number(parameters, "harmonics", 8.5),
    );
    put(&mut params, "scope", number(parameters, "scope", 7_500.0));
    put(&mut params, "ceil", number(parameters, "ceil", 16_000.0));
    put(&mut params, "blend", number(parameters, "blend", 0.0));
    put(
        &mut params,
        "ceil_active",
        flag(parameters, "ceil-active", false),
    );
    params
}

const VOICE_SUPPRESSOR_KEYS: &[&str] = &[
    "bypass",
    "freq-start",
    "freq-end",
    "correlation",
    "phase-difference",
    "inverted-mode",
    "maximum-instantaneous-frequency",
];

pub(super) fn voice_suppressor(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(
        parameters,
        VOICE_SUPPRESSOR_KEYS,
        "voice suppressor",
        warnings,
    );
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "start",
        number(parameters, "freq-start", 200.0),
    );
    put(&mut params, "end", number(parameters, "freq-end", 8_000.0));
    put(
        &mut params,
        "correlation",
        number(parameters, "correlation", 0.5),
    );
    put(
        &mut params,
        "phase",
        number(parameters, "phase-difference", 30.0),
    );
    put(
        &mut params,
        "inverted",
        flag(parameters, "inverted-mode", false),
    );
    put(
        &mut params,
        "max_if",
        number(parameters, "maximum-instantaneous-frequency", 12_000.0),
    );
    params
}

const NOISE_REDUCTION_KEYS: &[&str] = &[
    "bypass",
    "inputGain",
    "outputGain",
    "enableVad",
    "vadThres",
    "wet",
];

pub(super) fn noise_reduction(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(
        parameters,
        NOISE_REDUCTION_KEYS,
        "noise reduction",
        warnings,
    );
    let mut params = BTreeMap::new();
    put(
        &mut params,
        "input_gain",
        number(parameters, "inputGain", 0.0),
    );
    put(
        &mut params,
        "output_gain",
        number(parameters, "outputGain", 0.0),
    );
    put(&mut params, "mix", mix_percent(parameters, "wet", 0.0));
    put(&mut params, "vad", flag(parameters, "enableVad", false));
    put(
        &mut params,
        "vad_threshold",
        number(parameters, "vadThres", 50.0),
    );
    params
}

#[cfg(test)]
mod tests;
