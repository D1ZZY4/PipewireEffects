use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::model::PresetValue;

pub(super) fn convert_parameters(
    kind: &str,
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    let mut params = BTreeMap::new();
    match kind {
        "filter" => {
            let mode = match text(parameters, "type") {
                "Low-pass" => 0.0,
                "High-pass" => 1.0,
                "Band-pass" => 2.0,
                "Band-reject" => 3.0,
                "Notch" => 3.0,
                "All-pass" => 4.0,
                "Bell" => 5.0,
                "Low Shelf" => 6.0,
                "High Shelf" => 7.0,
                other => {
                    warnings.push(format!("filter type '{other}' was mapped to low pass"));
                    0.0
                }
            };
            put(&mut params, "mode", mode);
            put(
                &mut params,
                "frequency",
                number(parameters, "frequency", 1_000.0),
            );
            put(
                &mut params,
                "q",
                number(parameters, "quality", std::f64::consts::FRAC_1_SQRT_2),
            );
            put(&mut params, "gain", number(parameters, "gain", 0.0));
            let slope = match text(parameters, "slope") {
                "x1" => 1.0,
                "x2" => 2.0,
                "x3" => 3.0,
                "x4" => 4.0,
                _ => 2.0,
            };
            put(&mut params, "order", slope);
        }
        "equalizer" => {
            let gains = band_gains(parameters, warnings);
            for (band, gain) in gains.iter().enumerate() {
                put(&mut params, band_id(band), f64::from(*gain));
            }
            let gain =
                number(parameters, "input-gain", 0.0) + number(parameters, "output-gain", 0.0);
            put(&mut params, "output_gain", gain);
            put(&mut params, "bandwidth", 1.414);
        }
        "bass_enhancer" => {
            put(&mut params, "frequency", number(parameters, "scope", 150.0));
            put(
                &mut params,
                "amount",
                number(parameters, "amount", 30.0) * 10.0,
            );
            put(&mut params, "gain", number(parameters, "output-gain", 0.0));
        }
        "deesser" => {
            put(
                &mut params,
                "frequency",
                number(parameters, "f1-freq", 6_000.0),
            );
            put(
                &mut params,
                "threshold",
                number(parameters, "threshold", -20.0),
            );
            put(&mut params, "ratio", number(parameters, "ratio", 4.0));
            put(
                &mut params,
                "mode",
                if text(parameters, "mode") == "Wideband" {
                    1.0
                } else {
                    0.0
                },
            );
            put(&mut params, "attack", 5.0);
            put(&mut params, "release", 100.0);
        }
        "compressor" => {
            put(
                &mut params,
                "threshold",
                number(parameters, "threshold", -18.0),
            );
            put(&mut params, "ratio", number(parameters, "ratio", 4.0));
            put(
                &mut params,
                "knee",
                number(parameters, "knee", 3.0).max(0.0),
            );
            put(&mut params, "attack", number(parameters, "attack", 20.0));
            put(&mut params, "release", number(parameters, "release", 200.0));
            put(&mut params, "makeup", number(parameters, "makeup", 0.0));
            put(&mut params, "mix", wet_percent(parameters));
            let detector = parameters
                .get("sidechain")
                .and_then(|sidechain| sidechain.get("mode"))
                .and_then(Value::as_str)
                .unwrap_or("Peak");
            put(
                &mut params,
                "detector",
                if detector == "RMS" { 1.0 } else { 0.0 },
            );
        }
        "limiter" => {
            put(&mut params, "limit", number(parameters, "threshold", -1.0));
            put(&mut params, "release", number(parameters, "release", 60.0));
            put(
                &mut params,
                "input_gain",
                number(parameters, "input-gain", 0.0),
            );
        }
        "reverb" => {
            put(
                &mut params,
                "pre_delay",
                number(parameters, "predelay", 20.0),
            );
            put(
                &mut params,
                "room_size",
                number(parameters, "decay-time", 1.0) * 40.0,
            );
            put(
                &mut params,
                "damping",
                number(parameters, "hf-damp", 6_000.0) / 200.0,
            );
            put(&mut params, "wet", number(parameters, "amount", -6.0));
            put(&mut params, "dry", number(parameters, "dry", 0.0));
            put(&mut params, "width", 100.0);
        }
        "stereo_tools" => {
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
                "stereo_base",
                number(parameters, "stereo-base", 0.0) * 1000.0,
            );
            put(
                &mut params,
                "stereo_width",
                number(parameters, "side-level", 0.5) * 200.0,
            );
        }
        "gate" => {
            put(
                &mut params,
                "threshold",
                number(parameters, "threshold", -40.0),
            );
            put(&mut params, "range", number(parameters, "range", 40.0));
            put(&mut params, "attack", number(parameters, "attack", 5.0));
            put(&mut params, "release", number(parameters, "release", 300.0));
            put(&mut params, "hold", number(parameters, "hold", 50.0));
        }
        "autogain" => {
            put(&mut params, "target", number(parameters, "target", -14.0));
            put(&mut params, "max_gain", 20.0);
            put(&mut params, "speed", 1_000.0);
            put(&mut params, "silence_threshold", -50.0);
        }
        "delay" => {
            put(
                &mut params,
                "delay_left",
                number(parameters, "delay", 200.0),
            );
            put(
                &mut params,
                "delay_right",
                number(parameters, "delay", 200.0),
            );
            put(
                &mut params,
                "feedback",
                number(parameters, "feedback", 30.0),
            );
            put(&mut params, "mix", wet_percent(parameters));
        }
        "crystalizer" => {
            put(&mut params, "amount", 30.0);
            put(&mut params, "gain", number(parameters, "output-gain", 0.0));
        }
        other => warnings.push(format!("'{other}' has no parameter mapping")),
    }
    params
}

fn band_gains(parameters: &Map<String, Value>, warnings: &mut Vec<String>) -> [f32; 10] {
    let source = parameters
        .get("left")
        .and_then(Value::as_object)
        .or(Some(parameters));
    let Some(bands) = source else {
        warnings.push("the equalizer has no band block".to_owned());
        return [0.0; 10];
    };
    let mut collected: Vec<(f32, f32)> = Vec::new();
    for (key, value) in bands {
        if !key.starts_with("band") {
            continue;
        }
        let Some(band) = value.as_object() else {
            continue;
        };
        let frequency = band.get("frequency").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let gain = band.get("gain").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let kind = band.get("type").and_then(Value::as_str).unwrap_or("Bell");
        if frequency <= 0.0 || kind.contains("Pass") || kind == "Notch" {
            continue;
        }
        collected.push((frequency, gain));
    }
    let targets = [
        31.5f32, 62.5, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
    ];
    let mut gains = [0.0f32; 10];
    for (position, target) in targets.iter().enumerate() {
        let mut best: Option<(f32, f32)> = None;
        for (frequency, gain) in &collected {
            let distance = (frequency / target).ln().abs();
            if best.is_none_or(|(closest, _)| distance < closest) {
                best = Some((distance, *gain));
            }
        }
        if let Some((distance, gain)) = best
            && distance < 1.0
        {
            gains[position] = gain;
        }
    }
    if collected.len() > targets.len() {
        warnings.push(format!(
            "{} equalizer bands were folded into {} fixed bands",
            collected.len(),
            targets.len()
        ));
    }
    gains
}

fn band_id(band: usize) -> &'static str {
    const IDS: [&str; 10] = [
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
    IDS[band.min(9)]
}

fn wet_percent(parameters: &Map<String, Value>) -> f64 {
    let wet = number(parameters, "wet", 0.0);
    if wet <= -80.0 {
        100.0
    } else {
        (100.0 * 10f64.powf(wet / 20.0)).clamp(0.0, 100.0)
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
