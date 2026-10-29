use std::collections::BTreeMap;

use pwe_core::i18n::{t, tr};
use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::equalizer::{DEFAULT_BANDWIDTH, fold, gain_id};
use super::{level_percent, number, put, skipped, skipped_with_bands, text};

const FILTER_KEYS: &[&str] = &["bypass", "type", "frequency", "quality", "gain", "slope"];

const EQUALIZER_KEYS: &[&str] = &["bypass", "input-gain", "output-gain", "left", "right"];

const BASS_ENHANCER_KEYS: &[&str] = &["bypass", "output-gain", "scope", "amount"];

const CRYSTALIZER_KEYS: &[&str] = &["bypass", "output-gain"];

const STEREO_TOOLS_KEYS: &[&str] = &["bypass", "input-gain", "balance-out", "side-level"];

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
    put(
        &mut params,
        "balance",
        number(parameters, "balance-out", 0.0) * 100.0,
    );
    put(
        &mut params,
        "stereo_width",
        level_percent(number(parameters, "side-level", 0.0)),
    );
    params
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
mod tests {
    use serde_json::json;

    use super::*;

    type Converter = fn(&Map<String, Value>, &mut Vec<String>) -> BTreeMap<String, PresetValue>;

    fn convert(effect: Converter, value: Value) -> (BTreeMap<String, PresetValue>, Vec<String>) {
        let parameters = value.as_object().cloned().unwrap_or_default();
        let mut warnings = Vec::new();
        let params = effect(&parameters, &mut warnings);
        (params, warnings)
    }

    fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
        match params.get(key) {
            Some(PresetValue::Float(value)) => *value,
            _ => f64::NAN,
        }
    }

    #[test]
    fn every_shelf_and_bell_type_reaches_its_mode() {
        for (label, mode) in [
            ("Low-pass", 0.0),
            ("High-pass", 1.0),
            ("Band-pass", 2.0),
            ("Notch", 3.0),
            ("Band-reject", 3.0),
            ("All-pass", 4.0),
            ("Bell", 5.0),
            ("Resonance", 5.0),
            ("Low-shelf", 6.0),
            ("High-shelf", 7.0),
        ] {
            let (params, warnings) = convert(filter, json!({ "type": label }));
            assert!((as_float(&params, "mode") - mode).abs() < 1.0e-6, "{label}");
            assert!(warnings.is_empty(), "{label}: {warnings:?}");
        }
    }

    #[test]
    fn an_unknown_filter_type_is_reported_and_falls_back() {
        let (params, warnings) = convert(filter, json!({ "type": "Oval" }));
        assert_eq!(as_float(&params, "mode"), 0.0);
        assert!(warnings.iter().any(|warning| warning.contains("Oval")));
    }

    #[test]
    fn the_ladder_types_keep_the_closest_family() {
        let (pass, warnings) = convert(filter, json!({ "type": "Ladder-pass" }));
        assert_eq!(as_float(&pass, "mode"), 0.0);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("Ladder-pass")),
            "{warnings:?}"
        );
        let (rejection, warnings) = convert(filter, json!({ "type": "Ladder-rejection" }));
        assert_eq!(as_float(&rejection, "mode"), 3.0);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("Ladder-rejection")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_missing_quality_becomes_the_butterworth_value() {
        let (params, _) = convert(filter, json!({ "frequency": 500.0 }));
        assert!((as_float(&params, "q") - std::f64::consts::FRAC_1_SQRT_2).abs() < 1.0e-9);
        assert_eq!(as_float(&params, "order"), 1.0);
    }

    #[test]
    fn a_steep_slope_is_reported_and_capped() {
        let (params, warnings) = convert(filter, json!({ "slope": "x8" }));
        assert_eq!(as_float(&params, "order"), 4.0);
        assert!(warnings.iter().any(|warning| warning.contains("x8")));
    }

    #[test]
    fn the_equalizer_sums_its_two_gain_stages() {
        let (params, _) = convert(
            equalizer,
            json!({ "input-gain": -2.5, "output-gain": 1.5, "left": {} }),
        );
        assert!((as_float(&params, "output_gain") + 1.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_bass_enhancer_amount_is_a_level_not_a_percentage() {
        let (params, _) = convert(bass_enhancer, json!({ "amount": 0.0 }));
        assert!((as_float(&params, "amount") - 100.0).abs() < 0.1);
        let (absent, _) = convert(bass_enhancer, json!({ "scope": 120.0 }));
        assert!((as_float(&absent, "amount") - 100.0).abs() < 0.1);
        let (silent, _) = convert(bass_enhancer, json!({ "amount": -100.0 }));
        assert_eq!(as_float(&silent, "amount"), 0.0);
    }

    #[test]
    fn the_crystalizer_keeps_its_own_amount() {
        let (params, warnings) = convert(
            crystalizer,
            json!({ "output-gain": -1.0, "adaptive-intensity": true }),
        );
        assert!(!params.contains_key("amount"));
        assert_eq!(as_float(&params, "gain"), -1.0);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("adaptive-intensity"))
        );
    }

    #[test]
    fn the_crystalizer_bands_are_reported_instead_of_dropped() {
        let (_, warnings) = convert(
            crystalizer,
            json!({
                "output-gain": 0.0,
                "band0": { "intensity": 0.5 },
                "band1": { "mute": false },
                "band12": { "bypass": true }
            }),
        );
        assert!(
            warnings.iter().any(|warning| warning.contains("band*")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_unity_side_level_keeps_the_stereo_image() {
        let (params, _) = convert(stereo_tools, json!({ "side-level": 0.0 }));
        assert!((as_float(&params, "stereo_width") - 100.0).abs() < 0.1);
        let (wider, _) = convert(stereo_tools, json!({ "side-level": 6.0 }));
        assert!((as_float(&wider, "stereo_width") - 199.5).abs() < 0.6);
        let (narrow, _) = convert(stereo_tools, json!({ "side-level": -6.0 }));
        assert!((as_float(&narrow, "stereo_width") - 50.1).abs() < 0.6);
    }
}
