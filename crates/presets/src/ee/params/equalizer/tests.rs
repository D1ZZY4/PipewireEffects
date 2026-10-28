use serde_json::{Map, Value, json};

use super::{DEFAULT_BANDWIDTH, Folding, fold, gain_id};

fn fold_value(value: Value) -> (Folding, Vec<String>) {
    let parameters = value.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let folding = fold(&parameters, &mut warnings);
    (folding, warnings)
}

#[test]
fn a_bell_band_reaches_the_fixed_bands_around_it() {
    let (folding, warnings) = fold_value(json!({
        "left": { "band0": { "frequency": 1000.0, "gain": 6.0, "q": 1.5, "type": "Bell" } }
    }));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!((folding.gains[5] - 6.0).abs() < 1.0e-6);
    assert!((folding.bandwidth - 1.5).abs() < 1.0e-6);
}

#[test]
fn a_muted_band_contributes_no_gain() {
    let (folding, warnings) = fold_value(json!({
        "left": { "band0": { "frequency": 1000.0, "gain": 6.0, "type": "Bell", "mute": true } }
    }));
    assert_eq!(folding.gains[5], 0.0);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("muted or non-soloed")),
        "{warnings:?}"
    );
}

#[test]
fn only_soloed_bands_contribute_when_one_is_soloed() {
    let (folding, warnings) = fold_value(json!({
        "left": {
            "band0": { "frequency": 1000.0, "gain": 6.0, "type": "Bell", "solo": true },
            "band1": { "frequency": 4000.0, "gain": -4.0, "type": "Bell" }
        }
    }));
    assert!((folding.gains[5] - 6.0).abs() < 1.0e-6);
    assert_eq!(folding.gains[8], 0.0);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("muted or non-soloed")),
        "{warnings:?}"
    );
}

#[test]
fn per_band_settings_without_an_equivalent_are_reported() {
    let (_, warnings) = fold_value(json!({
        "left": { "band0": {
            "frequency": 1000.0, "gain": 3.0, "type": "Bell",
            "mode": "RLC (BT)", "slope": "x2", "width": 400.0
        } }
    }));
    let reported = warnings.join(" ");
    assert!(reported.contains("mode"), "{reported}");
    assert!(reported.contains("slope"), "{reported}");
    assert!(reported.contains("width"), "{reported}");
}

#[test]
fn a_band_without_a_frequency_is_reported() {
    let (folding, warnings) = fold_value(json!({
        "left": { "band0": { "gain": 6.0, "type": "Bell" } }
    }));
    assert_eq!(folding.gains[5], 0.0);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("usable frequency")),
        "{warnings:?}"
    );
}

#[test]
fn shelves_carry_their_gain() {
    let (folding, _) = fold_value(json!({
        "left": {
            "band0": { "frequency": 70.0, "gain": 1.8, "type": "Lo-shelf" },
            "band1": { "frequency": 16000.0, "gain": 0.5, "type": "Hi-shelf" }
        }
    }));
    assert!((folding.gains[1] - 1.8).abs() < 1.0e-6);
    assert!((folding.gains[9] - 0.5).abs() < 1.0e-6);
}

#[test]
fn pass_notch_and_disabled_bands_are_skipped_with_a_warning() {
    let (folding, warnings) = fold_value(json!({
        "left": {
            "band0": { "frequency": 100.0, "gain": 3.0, "type": "Hi-pass" },
            "band1": { "frequency": 1000.0, "gain": 3.0, "type": "Notch" },
            "band2": { "frequency": 500.0, "gain": 3.0, "type": "Off" }
        }
    }));
    assert!(folding.gains.iter().all(|gain| *gain == 0.0));
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("3 band types")),
        "{warnings:?}"
    );
}

#[test]
fn a_flat_legacy_document_folds_without_a_channel_block() {
    let (folding, _) = fold_value(json!({
        "band3": { "frequency": 250.0, "gain": 4.0, "type": "Bell" }
    }));
    assert!((folding.gains[3] - 4.0).abs() < 1.0e-6);
}

#[test]
fn a_shared_curve_folds_both_channel_blocks() {
    let (folding, warnings) = fold_value(json!({
        "left": {
            "band0": { "frequency": 1000.0, "gain": 3.0, "type": "Bell" }
        },
        "right": {
            "band0": { "frequency": 1000.0, "gain": 3.0, "type": "Bell" },
            "band1": { "frequency": 4000.0, "gain": -2.0, "type": "Bell" }
        }
    }));
    assert!((folding.gains[5] - 3.0).abs() < 1.0e-6);
    assert!((folding.gains[8] + 2.0).abs() < 1.0e-6);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_right_channel_that_differs_is_reported() {
    let (folding, warnings) = fold_value(json!({
        "left": { "band0": { "frequency": 1000.0, "gain": 3.0, "type": "Bell" } },
        "right": { "band0": { "frequency": 1000.0, "gain": -3.0, "type": "Bell" } }
    }));
    assert!((folding.gains[5] - 3.0).abs() < 1.0e-6);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("right-channel")),
        "{warnings:?}"
    );
}

#[test]
fn too_many_bands_are_reported() {
    let mut bands = Map::new();
    for band in 0..12 {
        bands.insert(
            format!("band{band}"),
            json!({ "frequency": 31.5 * f64::powi(2.0, band), "gain": 1.0, "type": "Bell" }),
        );
    }
    let (_, warnings) = fold_value(Value::Object(bands));
    assert!(
        warnings.iter().any(|warning| warning.contains("folded")),
        "{warnings:?}"
    );
}

#[test]
fn the_bandwidth_falls_back_when_no_quality_is_stored() {
    let (folding, _) = fold_value(json!({ "band0": { "frequency": 1000.0, "gain": 1.0 } }));
    assert!((folding.bandwidth - DEFAULT_BANDWIDTH).abs() < 1.0e-9);
    assert_eq!(gain_id(99), "band10_gain");
}
