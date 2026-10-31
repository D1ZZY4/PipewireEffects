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
fn the_side_level_is_transferred_in_decibels() {
    let (params, _) = convert(stereo_tools, json!({ "side-level": 0.0 }));
    assert!(as_float(&params, "side_level").abs() < 1.0e-6);
    let (wider, _) = convert(stereo_tools, json!({ "side-level": 6.0 }));
    assert!((as_float(&wider, "side_level") - 6.0).abs() < 1.0e-6);
    let (narrow, _) = convert(stereo_tools, json!({ "side-level": -6.0 }));
    assert!((as_float(&narrow, "side_level") + 6.0).abs() < 1.0e-6);
}

#[test]
fn the_stereo_routing_mode_and_the_switches_are_transferred() {
    let (params, _) = convert(
        stereo_tools,
        json!({
            "mode": "LR > L+R (Mono Sum L+R)",
            "balance-in": 0.5,
            "middle-panorama": -0.25,
            "side-balance": 0.75,
            "middle-level": 3.0,
            "softclip": true,
            "phasel": true,
            "muter": true,
        }),
    );
    assert!((as_float(&params, "mode") - 5.0).abs() < 1.0e-6);
    assert!((as_float(&params, "balance_in") - 50.0).abs() < 1.0e-6);
    assert!((as_float(&params, "middle_panorama") + 25.0).abs() < 1.0e-6);
    assert!((as_float(&params, "side_balance") - 75.0).abs() < 1.0e-6);
    assert!((as_float(&params, "middle_level") - 3.0).abs() < 1.0e-6);
    assert_eq!(as_float(&params, "softclip"), 1.0);
    assert_eq!(as_float(&params, "phase_left"), 1.0);
    assert_eq!(as_float(&params, "mute_right"), 1.0);
}
