use crate::ee;
use crate::error::PresetResult;
use crate::model::Preset;

struct Builtin {
    name: &'static str,
    text: &'static str,
}

const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "Default",
        text: include_str!("../../../../data/presets/default.json"),
    },
    Builtin {
        name: "Podcast voice",
        text: include_str!("../../../../data/presets/podcast-voice.json"),
    },
    Builtin {
        name: "Music, speakers",
        text: include_str!("../../../../data/presets/music-speakers.json"),
    },
    Builtin {
        name: "Headphones",
        text: include_str!("../../../../data/presets/headphones.json"),
    },
    Builtin {
        name: "Bass boost",
        text: include_str!("../../../../data/presets/bass-boost.json"),
    },
    Builtin {
        name: "Clean microphone",
        text: include_str!("../../../../data/presets/clean-microphone.json"),
    },
];

pub fn names() -> Vec<&'static str> {
    BUILTINS.iter().map(|builtin| builtin.name).collect()
}

pub fn find(name: &str) -> Option<PresetResult<(Preset, Vec<String>)>> {
    BUILTINS
        .iter()
        .find(|builtin| builtin.name == name)
        .map(|builtin| parse(builtin.text, builtin.name))
}

pub fn parse(text: &str, name: &str) -> PresetResult<(Preset, Vec<String>)> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    if ee::is_ees(&value) {
        let (preset, warnings) =
            ee::convert(&value, name).map_err(crate::error::PresetError::InvalidValue)?;
        return Ok((preset, warnings));
    }
    let preset: Preset = serde_json::from_value(value)?;
    preset.validate()?;
    Ok((preset, Vec::new()))
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
