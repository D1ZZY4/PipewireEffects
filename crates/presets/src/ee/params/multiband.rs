use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::{flag, number, put, skipped};

const BANDS: usize = 4;
const REFERENCE_BANDS: usize = 8;

const COMPRESSOR_GLOBAL: &[&str] = &[
    "bypass",
    "input-gain",
    "output-gain",
    "compressor-mode",
    "envelope-boost",
    "stereo-split",
    "input-to-sidechain",
    "input-to-link",
    "sidechain-to-input",
    "sidechain-to-link",
    "link-to-input",
    "link-to-sidechain",
];

const COMPRESSOR_BAND: &[&str] = &[
    "enable-band",
    "split-frequency",
    "compressor-enable",
    "mute",
    "solo",
    "attack-threshold",
    "attack-time",
    "release-threshold",
    "release-time",
    "ratio",
    "knee",
    "makeup",
    "compression-mode",
    "sidechain-type",
    "sidechain-mode",
    "sidechain-source",
    "stereo-split-source",
    "sidechain-lookahead",
    "sidechain-reactivity",
    "sidechain-preamp",
    "sidechain-custom-lowcut-filter",
    "sidechain-custom-highcut-filter",
    "sidechain-lowcut-frequency",
    "sidechain-highcut-frequency",
    "boost-threshold",
    "boost-amount",
];

const GATE_GLOBAL: &[&str] = &[
    "bypass",
    "input-gain",
    "output-gain",
    "gate-mode",
    "envelope-boost",
    "stereo-split",
    "input-to-sidechain",
    "input-to-link",
    "sidechain-to-input",
    "sidechain-to-link",
    "link-to-input",
    "link-to-sidechain",
];

const GATE_BAND: &[&str] = &[
    "enable-band",
    "split-frequency",
    "gate-enable",
    "mute",
    "solo",
    "attack-time",
    "release-time",
    "curve-threshold",
    "curve-zone",
    "hysteresis",
    "hysteresis-threshold",
    "hysteresis-zone",
    "reduction",
    "makeup",
    "sidechain-type",
    "sidechain-mode",
    "sidechain-source",
    "stereo-split-source",
    "sidechain-lookahead",
    "sidechain-reactivity",
    "sidechain-preamp",
    "sidechain-custom-lowcut-filter",
    "sidechain-custom-highcut-filter",
    "sidechain-lowcut-frequency",
    "sidechain-highcut-frequency",
];

pub(super) fn multiband_compressor(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(
        parameters,
        COMPRESSOR_GLOBAL,
        "multiband compressor",
        warnings,
    );
    let mut params = BTreeMap::new();
    let count = band_count(parameters, "multiband compressor", warnings);
    put(&mut params, "bands", count as f64);
    splits(parameters, &mut params, count);
    for index in 0..count {
        let Some(band) = band_map(parameters, index) else {
            continue;
        };
        skipped(band, COMPRESSOR_BAND, "multiband compressor", warnings);
        let n = index + 1;
        put(
            &mut params,
            &format!("band{n}_enabled"),
            flag(band, "compressor-enable", true),
        );
        put(
            &mut params,
            &format!("band{n}_threshold"),
            number(band, "attack-threshold", -18.0),
        );
        put(
            &mut params,
            &format!("band{n}_ratio"),
            number(band, "ratio", 2.0),
        );
        put(
            &mut params,
            &format!("band{n}_knee"),
            number(band, "knee", 6.0).abs(),
        );
        put(
            &mut params,
            &format!("band{n}_attack"),
            number(band, "attack-time", 20.0),
        );
        put(
            &mut params,
            &format!("band{n}_release"),
            number(band, "release-time", 200.0),
        );
        put(
            &mut params,
            &format!("band{n}_makeup"),
            number(band, "makeup", 0.0),
        );
        mode(band, "compression-mode", "multiband compressor", warnings);
    }
    params
}

pub(super) fn multiband_gate(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, GATE_GLOBAL, "multiband gate", warnings);
    let mut params = BTreeMap::new();
    let count = band_count(parameters, "multiband gate", warnings);
    put(&mut params, "bands", count as f64);
    splits(parameters, &mut params, count);
    for index in 0..count {
        let Some(band) = band_map(parameters, index) else {
            continue;
        };
        skipped(band, GATE_BAND, "multiband gate", warnings);
        let n = index + 1;
        put(
            &mut params,
            &format!("band{n}_enabled"),
            flag(band, "gate-enable", true),
        );
        put(
            &mut params,
            &format!("band{n}_threshold"),
            number(band, "curve-threshold", -40.0),
        );
        put(
            &mut params,
            &format!("band{n}_range"),
            range(band, warnings),
        );
        put(
            &mut params,
            &format!("band{n}_attack"),
            number(band, "attack-time", 20.0),
        );
        put(
            &mut params,
            &format!("band{n}_release"),
            number(band, "release-time", 100.0),
        );
        put(
            &mut params,
            &format!("band{n}_makeup"),
            number(band, "makeup", 0.0),
        );
    }
    params
}

fn band_map(parameters: &Map<String, Value>, index: usize) -> Option<&Map<String, Value>> {
    parameters
        .get(&format!("band{index}"))
        .and_then(Value::as_object)
}

fn band_count(parameters: &Map<String, Value>, effect: &str, warnings: &mut Vec<String>) -> usize {
    let mut count = 1usize;
    for index in 1..REFERENCE_BANDS {
        let enabled = band_map(parameters, index)
            .and_then(|band| band.get("enable-band"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if enabled {
            count = count.max(index + 1);
        }
    }
    if count > BANDS {
        warnings.push(tr(
            "{0}: this build splits up to {1} bands and the rest were dropped",
            &[effect, &BANDS.to_string()],
        ));
        count = BANDS;
    }
    count.max(2)
}

fn splits(
    parameters: &Map<String, Value>,
    params: &mut BTreeMap<String, PresetValue>,
    count: usize,
) {
    for index in 1..count {
        let value = band_map(parameters, index)
            .and_then(|band| band.get("split-frequency"))
            .and_then(Value::as_f64);
        if let Some(value) = value {
            put(params, &format!("split{index}"), value);
        }
    }
}

fn range(band: &Map<String, Value>, warnings: &mut Vec<String>) -> f64 {
    let reduction = number(band, "reduction", -24.0);
    if reduction > 0.0 {
        warnings.push(tr(
            "multiband gate: the reverse mode has no equivalent here and was left closed",
            &[],
        ));
        0.0
    } else {
        -reduction
    }
}

fn mode(band: &Map<String, Value>, key: &str, effect: &str, warnings: &mut Vec<String>) {
    match band.get(key).and_then(Value::as_str) {
        None | Some("") | Some("Downward") => {}
        Some(other) => warnings.push(tr(
            "{0}: the band mode '{1}' has no equivalent here and the downward curve was loaded",
            &[effect, other],
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parameters(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn the_split_frequencies_and_band_values_transfer() {
        let parameters = parameters(json!({
            "compressor-mode": "Classic",
            "band0": { "compressor-enable": true, "attack-threshold": -30.0, "ratio": 4.0 },
            "band1": { "enable-band": true, "split-frequency": 250.0, "attack-threshold": -24.0, "ratio": 3.0 },
            "band2": { "enable-band": true, "split-frequency": 2500.0, "attack-threshold": -18.0 }
        }));
        let mut warnings = Vec::new();
        let params = multiband_compressor(&parameters, &mut warnings);
        assert_eq!(params.get("bands"), Some(&PresetValue::Float(3.0)));
        assert_eq!(params.get("split1"), Some(&PresetValue::Float(250.0)));
        assert_eq!(params.get("split2"), Some(&PresetValue::Float(2500.0)));
        assert_eq!(
            params.get("band2_threshold"),
            Some(&PresetValue::Float(-24.0))
        );
    }

    #[test]
    fn more_than_four_bands_are_reported() {
        let mut value = Map::new();
        for index in 0..8 {
            value.insert(
                format!("band{index}"),
                json!({ "enable-band": true, "split-frequency": 100.0 * (index as f64 + 1.0) }),
            );
        }
        let mut warnings = Vec::new();
        let params = multiband_compressor(&parameters(Value::Object(value)), &mut warnings);
        assert_eq!(params.get("bands"), Some(&PresetValue::Float(4.0)));
        assert!(
            warnings.iter().any(|warning| warning.contains("4 bands")),
            "{warnings:?}"
        );
    }

    #[test]
    fn the_gate_reduction_becomes_a_positive_range() {
        let parameters = parameters(json!({
            "band0": { "gate-enable": true, "curve-threshold": -30.0, "reduction": -24.0 }
        }));
        let mut warnings = Vec::new();
        let params = multiband_gate(&parameters, &mut warnings);
        assert_eq!(params.get("band1_range"), Some(&PresetValue::Float(24.0)));
        assert_eq!(
            params.get("band1_threshold"),
            Some(&PresetValue::Float(-30.0))
        );
    }
}
