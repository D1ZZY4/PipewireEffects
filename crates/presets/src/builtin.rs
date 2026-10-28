use crate::ee;
use crate::error::PresetResult;
use crate::model::Preset;

const DEFAULT_TEXT: &str = include_str!("../../../data/presets/default.json");
const DEFAULT_NAME: &str = "Default";

pub fn names() -> Vec<&'static str> {
    vec![DEFAULT_NAME]
}

pub fn find(name: &str) -> Option<PresetResult<(Preset, Vec<String>)>> {
    (name == DEFAULT_NAME).then(|| parse(DEFAULT_TEXT, DEFAULT_NAME))
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
mod tests {
    use super::*;

    #[test]
    fn the_builtin_preset_parses() {
        let (preset, warnings) = find("Default")
            .expect("the default preset is registered")
            .expect("the default preset parses");
        assert_eq!(preset.name, "Default");
        assert!(!preset.output.effects.is_empty());
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn unknown_names_are_reported() {
        assert!(find("Nope").is_none());
        assert!(names().contains(&"Default"));
    }
}
