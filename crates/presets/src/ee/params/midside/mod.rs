use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::model::PresetValue;

use super::equalizer::fold;
use super::equalizer::gain_id;
use super::{number, put, skipped};

const MIDSIDE_KEYS: &[&str] = &["bypass", "output-gain", "mid", "side"];

pub(super) fn midside_equalizer(
    parameters: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PresetValue> {
    skipped(parameters, MIDSIDE_KEYS, "midside_equalizer", warnings);
    let mut params = BTreeMap::new();
    let mut bandwidth = None;
    for (block, prefix) in [("mid", "mid_"), ("side", "side_")] {
        let Some(channel) = parameters.get(block).and_then(Value::as_object) else {
            continue;
        };
        let mut map = Map::new();
        map.insert("left".to_owned(), Value::Object(channel.clone()));
        let folded = fold(&map, "midside_equalizer", warnings);
        for (band, gain) in folded.gains.iter().enumerate() {
            put(
                &mut params,
                &format!("{prefix}{}", gain_id(band)),
                f64::from(*gain),
            );
        }
        bandwidth = bandwidth.or(folded.bandwidth);
    }
    if let Some(bandwidth) = bandwidth {
        put(&mut params, "bandwidth", bandwidth);
    }
    put(
        &mut params,
        "output_gain",
        number(parameters, "output-gain", 0.0),
    );
    params
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
