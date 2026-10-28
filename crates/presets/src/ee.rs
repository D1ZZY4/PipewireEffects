mod params;

use serde_json::Value;

use crate::model::{EffectEntry, Pipeline, Preset, SCHEMA_VERSION};

use params::convert_parameters;

pub fn is_ees(value: &Value) -> bool {
    value.get("schema_version").is_none() && value.get("output").is_some()
}

pub fn convert(value: &Value, name: &str) -> Result<(Preset, Vec<String>), String> {
    let output = value
        .get("output")
        .and_then(Value::as_object)
        .ok_or_else(|| "the document has no output section".to_owned())?;
    let mut warnings = Vec::new();
    let mut pipeline = Pipeline::default();

    let order: Vec<String> = output
        .get("plugins_order")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    for instance in order {
        let Some(kind) = instance.split('#').next() else {
            continue;
        };
        let Some(mapped) = map_kind(kind) else {
            warnings.push(format!("'{kind}' has no equivalent here and was skipped"));
            continue;
        };
        let Some(parameters) = output.get(&instance).and_then(Value::as_object) else {
            warnings.push(format!("'{instance}' has no parameter block"));
            continue;
        };
        let enabled = !parameters
            .get("bypass")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let params = convert_parameters(mapped, parameters, &mut warnings);
        pipeline.effects.push(EffectEntry {
            kind: mapped.to_owned(),
            enabled,
            params,
        });
    }

    if pipeline.effects.is_empty() {
        return Err("no effect in the preset could be mapped".to_owned());
    }
    Ok((
        Preset {
            schema_version: SCHEMA_VERSION,
            name: name.to_owned(),
            output: pipeline,
        },
        warnings,
    ))
}

fn map_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "filter" => "filter",
        "equalizer" => "equalizer",
        "bass_enhancer" => "bass_enhancer",
        "deesser" => "deesser",
        "compressor" => "compressor",
        "limiter" => "limiter",
        "reverb" => "reverb",
        "stereo_tools" => "stereo_tools",
        "gate" => "gate",
        "autogain" => "autogain",
        "delay" => "delay",
        "crystalizer" => "crystalizer",
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
