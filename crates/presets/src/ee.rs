mod params;

use serde_json::{Map, Value};

use pwe_core::i18n::{t, tr};

use crate::model::{EffectEntry, Pipeline, Preset, SCHEMA_VERSION};

use params::convert_parameters;

pub fn is_ees(value: &Value) -> bool {
    value.get("schema_version").is_none()
        && (value.get("output").is_some() || value.get("input").is_some())
}

pub fn convert(value: &Value, name: &str) -> Result<(Preset, Vec<String>), String> {
    let mut warnings = Vec::new();
    let mut output = Pipeline::default();
    let mut input = Pipeline::default();
    let mut found = false;

    if let Some(section) = value.get("output").and_then(Value::as_object) {
        found = true;
        convert_section(section, "output", &mut output, &mut warnings);
    }
    if let Some(section) = value.get("input").and_then(Value::as_object) {
        found = true;
        convert_section(section, "input", &mut input, &mut warnings);
    }

    if !found {
        return Err(t("the document has no output section").to_owned());
    }
    if output.effects.is_empty() && input.effects.is_empty() {
        return Err(t("no effect in the preset could be mapped").to_owned());
    }
    Ok((
        Preset {
            schema_version: SCHEMA_VERSION,
            name: name.to_owned(),
            output,
            input,
        },
        warnings,
    ))
}

fn convert_section(
    section: &Map<String, Value>,
    label: &str,
    pipeline: &mut Pipeline,
    warnings: &mut Vec<String>,
) {
    let mut local = Vec::new();
    pipeline.bypassed = section
        .get("bypass")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let order: Vec<String> = section
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
        let kind = instance
            .split_once('#')
            .map_or(instance.as_str(), |(kind, _)| kind);
        let Some(mapped) = map_kind(kind) else {
            local.push(tr("'{0}' has no equivalent here and was skipped", &[kind]));
            continue;
        };
        let parameters = section
            .get(&instance)
            .or_else(|| section.get(kind))
            .and_then(Value::as_object);
        let Some(parameters) = parameters else {
            local.push(tr("'{0}' has no parameter block", &[&instance]));
            continue;
        };
        let enabled = !parameters
            .get("bypass")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let params = convert_parameters(mapped, parameters, &mut local);
        pipeline.effects.push(EffectEntry {
            kind: mapped.to_owned(),
            enabled,
            params,
        });
    }

    for warning in local {
        if label == "output" {
            warnings.push(warning);
        } else {
            warnings.push(tr("{0}: {1}", &[label, &warning]));
        }
    }
}

fn map_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "filter" => "filter",
        "equalizer" => "equalizer",
        "bass_enhancer" => "bass_enhancer",
        "bass_loudness" => "bass_loudness",
        "deesser" => "deesser",
        "compressor" => "compressor",
        "limiter" => "limiter",
        "reverb" => "reverb",
        "stereo_tools" => "stereo_tools",
        "gate" => "gate",
        "multiband_compressor" => "multiband_compressor",
        "multiband_gate" => "multiband_gate",
        "expander" => "expander",
        "maximizer" => "maximizer",
        "crosstalk_canceller" => "crosstalk",
        "voice_suppressor" => "voice_suppressor",
        "rnnoise" => "noise_reduction",
        "midside_equalizer" => "midside_equalizer",
        "exciter" => "exciter",
        "autogain" => "autogain",
        "loudness" => "loudness",
        "pitch" => "pitch_shift",
        "delay" => "delay",
        "crystalizer" => "crystalizer",
        "crusher" => "crusher",
        "crossfeed" => "crossfeed",
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
