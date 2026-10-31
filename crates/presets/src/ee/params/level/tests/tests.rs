use serde_json::json;

use super::*;

fn as_float(params: &BTreeMap<String, PresetValue>, key: &str) -> f64 {
    match params.get(key) {
        Some(PresetValue::Float(value)) => *value,
        _ => f64::NAN,
    }
}

fn convert(value: Value) -> (BTreeMap<String, PresetValue>, Vec<String>) {
    let parameters = value.as_object().cloned().unwrap_or_default();
    let mut warnings = Vec::new();
    let params = loudness(&parameters, &mut warnings);
    (params, warnings)
}

#[test]
fn the_contour_controls_are_transferred() {
    let (params, warnings) = convert(json!({
        "std": 2, "volume": -30.0, "clipping": true, "clippingRange": 12.0, "mode": 1
    }));
    assert!((as_float(&params, "standard") - 2.0).abs() < 1.0e-9);
    assert!((as_float(&params, "volume") + 30.0).abs() < 1.0e-9);
    assert_eq!(params.get("clipping"), Some(&PresetValue::Boolean(true)));
    assert!((as_float(&params, "clipping_range") - 12.0).abs() < 1.0e-9);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_reference_defaults_are_honoured_when_a_key_is_missing() {
    let (params, warnings) = convert(json!({}));
    assert!((as_float(&params, "standard") - 4.0).abs() < 1.0e-9);
    assert!((as_float(&params, "volume") - 0.0).abs() < 1.0e-9);
    assert!((as_float(&params, "clipping_range") - 6.0).abs() < 1.0e-9);
    assert_eq!(params.get("clipping"), Some(&PresetValue::Boolean(false)));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_fft_mode_is_reported_and_the_iir_mode_is_not() {
    let (_, fft) = convert(json!({ "mode": 0 }));
    assert!(fft.iter().any(|warning| warning.contains("FFT")), "{fft:?}");
    let (_, iir) = convert(json!({ "mode": 1 }));
    assert!(iir.is_empty(), "{iir:?}");
}

#[test]
fn the_fft_size_and_the_per_plugin_gains_are_reported() {
    let (_, warnings) = convert(json!({ "fft": 4096, "inputGain": -3.0 }));
    let joined = warnings.join(" ");
    assert!(joined.contains("fft"), "{joined}");
    assert!(joined.contains("inputGain"), "{joined}");
}
