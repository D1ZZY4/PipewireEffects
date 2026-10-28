use crate::error::PresetResult;
use crate::model::Preset;

const SOURCES: &[(&str, &str)] = &[
    (
        "Bass boost",
        include_str!("../../../data/presets/bass-boost.json"),
    ),
    ("Flat", include_str!("../../../data/presets/flat.json")),
    (
        "Loudness safe",
        include_str!("../../../data/presets/loudness-safe.json"),
    ),
    (
        "Night mode",
        include_str!("../../../data/presets/night-mode.json"),
    ),
    (
        "Podcast",
        include_str!("../../../data/presets/podcast.json"),
    ),
    (
        "Vocal clarity",
        include_str!("../../../data/presets/vocal-clarity.json"),
    ),
];

pub fn names() -> Vec<&'static str> {
    SOURCES.iter().map(|(name, _)| *name).collect()
}

pub fn find(name: &str) -> Option<PresetResult<Preset>> {
    SOURCES
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, text)| parse(text))
}

pub fn parse(text: &str) -> PresetResult<Preset> {
    let preset: Preset = serde_json::from_str(text)?;
    preset.validate()?;
    Ok(preset)
}

pub fn all() -> Vec<PresetResult<Preset>> {
    SOURCES.iter().map(|(_, text)| parse(text)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_parses_and_validates() {
        let builtins = all();
        assert_eq!(builtins.len(), SOURCES.len());
        for preset in builtins {
            let preset = preset.expect("builtin preset should parse");
            assert!(!preset.output.effects.is_empty() || preset.name == "Flat");
        }
    }

    #[test]
    fn unknown_names_are_reported() {
        assert!(find("Nope").is_none());
        assert!(names().contains(&"Flat"));
    }
}
