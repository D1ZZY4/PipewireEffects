use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

const TARGETS: [f32; 10] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

pub(super) const DEFAULT_BANDWIDTH: f64 = 1.414;
const MIN_BANDWIDTH: f64 = 0.3;
const MAX_BANDWIDTH: f64 = 6.0;

const IGNORED_BAND_KEYS: [&str; 3] = ["mode", "slope", "width"];

const BAND_IDS: [&str; 10] = [
    "band1_gain",
    "band2_gain",
    "band3_gain",
    "band4_gain",
    "band5_gain",
    "band6_gain",
    "band7_gain",
    "band8_gain",
    "band9_gain",
    "band10_gain",
];

pub(super) struct Folding {
    pub gains: [f32; 10],
    pub bandwidth: Option<f64>,
}

pub(super) fn fold(
    parameters: &Map<String, Value>,
    effect: &str,
    warnings: &mut Vec<String>,
) -> Folding {
    let mut bands: BTreeMap<&str, &Value> = BTreeMap::new();
    let left = parameters.get("left").and_then(Value::as_object);
    for (key, value) in left.unwrap_or(parameters) {
        if key.starts_with("band") {
            bands.insert(key.as_str(), value);
        }
    }
    if let Some(right) = parameters.get("right").and_then(Value::as_object) {
        merge_channel(&mut bands, right, effect, warnings);
    }

    let soloed = bands.values().any(|value| flag(value, "solo"));
    let mut collected: Vec<(f32, f32)> = Vec::new();
    let mut qualities: Vec<f64> = Vec::new();
    let mut unsupported = 0usize;
    let mut inactive = 0usize;
    let mut unusable = 0usize;
    let mut ignored: BTreeSet<&str> = BTreeSet::new();
    for value in bands.values() {
        let Some(band) = value.as_object() else {
            continue;
        };
        for key in IGNORED_BAND_KEYS {
            if band.contains_key(key) {
                ignored.insert(key);
            }
        }
        if !carries_gain(band.get("type").and_then(Value::as_str).unwrap_or("")) {
            unsupported += 1;
            continue;
        }
        if flag(value, "mute") || (soloed && !flag(value, "solo")) {
            inactive += 1;
            continue;
        }
        let frequency = band.get("frequency").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        if frequency <= 0.0 {
            unusable += 1;
            continue;
        }
        let gain = band.get("gain").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        collected.push((frequency, gain));
        if let Some(quality) = band.get("q").and_then(Value::as_f64) {
            qualities.push(quality);
        }
    }
    if collected.len() > TARGETS.len() {
        warnings.push(tr(
            "{0}: {1} bands were folded into {2} fixed bands",
            &[
                effect,
                &collected.len().to_string(),
                &TARGETS.len().to_string(),
            ],
        ));
    }
    if unsupported > 0 {
        warnings.push(tr(
            "{0}: {1} band types without an equivalent were skipped",
            &[effect, &unsupported.to_string()],
        ));
    }
    if inactive > 0 {
        warnings.push(tr(
            "{0}: {1} muted or non-soloed bands were skipped",
            &[effect, &inactive.to_string()],
        ));
    }
    if unusable > 0 {
        warnings.push(tr(
            "{0}: {1} bands without a usable frequency were skipped",
            &[effect, &unusable.to_string()],
        ));
    }
    if !ignored.is_empty() {
        warnings.push(tr(
            "{0}: the per-band {1} were ignored, because every band here is one filter with a shared bandwidth",
            &[effect, &ignored.into_iter().collect::<Vec<&str>>().join(", ")],
        ));
    }
    let (gains, unassigned) = nearest_gains(&collected);
    if unassigned > 0 {
        warnings.push(tr(
            "{0}: {1} bands fell outside the ten fixed bands and were skipped",
            &[effect, &unassigned.to_string()],
        ));
    }
    Folding {
        gains,
        bandwidth: bandwidth(&qualities),
    }
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn merge_channel<'a>(
    bands: &mut BTreeMap<&'a str, &'a Value>,
    channel: &'a Map<String, Value>,
    effect: &str,
    warnings: &mut Vec<String>,
) {
    let mut conflicts = 0usize;
    for (key, value) in channel {
        if !key.starts_with("band") {
            continue;
        }
        match bands.get(key.as_str()) {
            Some(existing) if !same_band(existing, value) => conflicts += 1,
            Some(_) => {}
            None => {
                bands.insert(key.as_str(), value);
            }
        }
    }
    if conflicts > 0 {
        warnings.push(tr(
            "{0}: {1} right-channel bands differ from the left channel and were merged into the shared curve",
            &[effect, &conflicts.to_string()],
        ));
    }
}

fn same_band(left: &Value, right: &Value) -> bool {
    ["type", "frequency", "gain", "q", "mute", "solo"]
        .iter()
        .all(|field| left.get(field) == right.get(field))
}

pub(super) fn gain_id(band: usize) -> &'static str {
    BAND_IDS[band.min(BAND_IDS.len() - 1)]
}

fn carries_gain(kind: &str) -> bool {
    matches!(
        kind,
        "" | "Bell" | "Resonance" | "Lo-shelf" | "Hi-shelf" | "Low Shelf" | "High Shelf"
    )
}

fn nearest_gains(collected: &[(f32, f32)]) -> ([f32; 10], usize) {
    let mut gains = [0.0f32; 10];
    let mut assigned = vec![false; collected.len()];
    for (position, target) in TARGETS.iter().enumerate() {
        let mut best: Option<(f32, usize)> = None;
        for (index, (frequency, _)) in collected.iter().enumerate() {
            let distance = (frequency / target).ln().abs();
            if best.is_none_or(|(closest, _)| distance < closest) {
                best = Some((distance, index));
            }
        }
        if let Some((distance, index)) = best
            && distance < 1.0
        {
            gains[position] = collected[index].1;
            assigned[index] = true;
        }
    }
    let unassigned = assigned.iter().filter(|flag| !**flag).count();
    (gains, unassigned)
}

fn bandwidth(qualities: &[f64]) -> Option<f64> {
    if qualities.is_empty() {
        return None;
    }
    let mut sorted = qualities.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let middle = sorted[sorted.len() / 2];
    middle
        .is_finite()
        .then(|| middle.clamp(MIN_BANDWIDTH, MAX_BANDWIDTH))
}

#[cfg(test)]
mod tests;
