use crate::ee;
use crate::error::PresetResult;
use crate::model::Preset;

const DEFAULT_TEXT: &str = include_str!("../../../data/presets/default.json");
const DEFAULT_NAME: &str = "Default";

pub fn names() -> Vec<&'static str> {
    vec![DEFAULT_NAME]
}

pub fn find(name: &str) -> Option<PresetResult<Preset>> {
    (name == DEFAULT_NAME).then(|| parse(DEFAULT_TEXT, DEFAULT_NAME))
}

pub fn parse(text: &str, name: &str) -> PresetResult<Preset> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    if ee::is_ees(&value) {
        let (preset, _) =
            ee::convert(&value, name).map_err(crate::error::PresetError::InvalidValue)?;
        return Ok(preset);
    }
    let preset: Preset = serde_json::from_value(value)?;
    preset.validate()?;
    Ok(preset)
}

pub fn all() -> Vec<PresetResult<Preset>> {
    vec![parse(DEFAULT_TEXT, DEFAULT_NAME)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_preset_parses() {
        let preset = all().remove(0).expect("the built-in preset should parse");
        assert_eq!(preset.name, "Default");
        assert!(!preset.output.effects.is_empty());
    }

    #[test]
    fn unknown_names_are_reported() {
        assert!(find("Nope").is_none());
        assert!(names().contains(&"Default"));
    }
}
