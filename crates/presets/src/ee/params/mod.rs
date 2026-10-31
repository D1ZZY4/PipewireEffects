mod colour;
mod dynamics;
mod equalizer;
mod harmony;
mod level;
mod loudness;
mod midside;
mod multiband;
mod pitch;
mod speech;
mod time;
mod tone;

use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

pub(super) fn convert_parameters(
    kind: &str,
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    match kind {
        "filter" => tone::filter(parameters, warnings),
        "equalizer" => tone::equalizer(parameters, warnings),
        "bass_enhancer" => tone::bass_enhancer(parameters, warnings),
        "bass_loudness" => loudness::bass_loudness(parameters, warnings),
        "crystalizer" => tone::crystalizer(parameters, warnings),
        "crusher" => colour::crusher(parameters, warnings),
        "crossfeed" => colour::crossfeed(parameters, warnings),
        "crosstalk" => colour::crosstalk(parameters, warnings),
        "voice_suppressor" => colour::voice_suppressor(parameters, warnings),
        "noise_reduction" => colour::noise_reduction(parameters, warnings),
        "speech_processor" => speech::speech_processor(parameters, warnings),
        "pitch_shift" => pitch::pitch_shift(parameters, warnings),
        "exciter" => colour::exciter(parameters, warnings),
        "expander" => dynamics::expander(parameters, warnings),
        "maximizer" => dynamics::maximizer(parameters, warnings),
        "midside_equalizer" => midside::midside_equalizer(parameters, warnings),
        "stereo_tools" => tone::stereo_tools(parameters, warnings),
        "compressor" => dynamics::compressor(parameters, warnings),
        "limiter" => dynamics::limiter(parameters, warnings),
        "gate" => dynamics::gate(parameters, warnings),
        "multiband_compressor" => multiband::multiband_compressor(parameters, warnings),
        "multiband_gate" => multiband::multiband_gate(parameters, warnings),
        "deesser" => dynamics::deesser(parameters, warnings),
        "autogain" => dynamics::autogain(parameters, warnings),
        "loudness" => level::loudness(parameters, warnings),
        "autotune" => harmony::autotune(parameters, warnings),
        "harmonizer" => harmony::harmonizer(parameters, warnings),
        "delay" => time::delay(parameters, warnings),
        "reverb" => time::reverb(parameters, warnings),
        other => {
            warnings.push(tr("'{0}' has no parameter mapping", &[other]));
            BTreeMap::new()
        }
    }
}

fn number(parameters: &Map<String, Value>, key: &str, fallback: f64) -> f64 {
    parameters
        .get(key)
        .and_then(Value::as_f64)
        .unwrap_or(fallback)
}

fn text<'a>(parameters: &'a Map<String, Value>, key: &str) -> &'a str {
    parameters.get(key).and_then(Value::as_str).unwrap_or("")
}

fn put(params: &mut BTreeMap<String, PresetValue>, key: &str, value: f64) {
    params.insert(key.to_owned(), PresetValue::Float(value));
}

fn flag(parameters: &Map<String, Value>, key: &str, fallback: bool) -> f64 {
    let missing = f64::from(fallback);
    match parameters.get(key) {
        Some(Value::Bool(value)) => f64::from(*value),
        Some(value) => value.as_f64().unwrap_or(missing),
        None => missing,
    }
}

fn level_percent(db: f64) -> f64 {
    if db <= -80.0 {
        0.0
    } else {
        100.0 * 10f64.powf(db / 20.0)
    }
}

fn mix_percent(parameters: &Map<String, Value>, key: &str, fallback_db: f64) -> f64 {
    level_percent(number(parameters, key, fallback_db))
}

fn skipped(
    parameters: &Map<String, Value>,
    handled: &[&str],
    effect: &str,
    warnings: &mut Vec<String>,
) {
    skipped_keys(parameters, handled, false, effect, warnings);
}

fn skipped_with_bands(
    parameters: &Map<String, Value>,
    handled: &[&str],
    effect: &str,
    warnings: &mut Vec<String>,
) {
    skipped_keys(parameters, handled, true, effect, warnings);
}

fn skipped_keys(
    parameters: &Map<String, Value>,
    handled: &[&str],
    bands_consumed: bool,
    effect: &str,
    warnings: &mut Vec<String>,
) {
    let mut unhandled: Vec<&str> = parameters
        .keys()
        .map(String::as_str)
        .filter(|key| !handled.contains(key))
        .filter(|key| !bands_consumed || !key.starts_with("band"))
        .collect();
    if unhandled.is_empty() {
        return;
    }
    unhandled.sort_unstable();
    warnings.push(tr(
        "{0}: {1} was left at its default",
        &[effect, &grouped(&unhandled)],
    ));
}

fn grouped(keys: &[&str]) -> String {
    let mut families: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut plain: Vec<&str> = Vec::new();
    for key in keys {
        match key_prefix(key) {
            Some(prefix) => families.entry(prefix).or_default().push(key),
            None => plain.push(key),
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for (prefix, members) in families {
        if members.len() > 1 {
            parts.push(format!("{prefix}*"));
        } else if let Some(member) = members.first() {
            parts.push((*member).to_owned());
        }
    }
    parts.extend(plain.into_iter().map(str::to_owned));
    parts.sort_unstable();
    parts.join(", ")
}

fn key_prefix(key: &str) -> Option<&str> {
    let digits = key.trim_end_matches(|character: char| character.is_ascii_digit());
    (digits.len() < key.len() && !digits.is_empty()).then_some(digits)
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
