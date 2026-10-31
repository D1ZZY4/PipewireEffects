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
        text: include_str!("../../../data/presets/default.json"),
    },
    Builtin {
        name: "Podcast voice",
        text: include_str!("../../../data/presets/podcast-voice.json"),
    },
    Builtin {
        name: "Music, speakers",
        text: include_str!("../../../data/presets/music-speakers.json"),
    },
    Builtin {
        name: "Headphones",
        text: include_str!("../../../data/presets/headphones.json"),
    },
    Builtin {
        name: "Bass boost",
        text: include_str!("../../../data/presets/bass-boost.json"),
    },
    Builtin {
        name: "Clean microphone",
        text: include_str!("../../../data/presets/clean-microphone.json"),
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
mod tests {
    use super::*;

    #[test]
    fn every_builtin_preset_parses_and_carries_its_own_name() {
        for name in names() {
            let (preset, warnings) = find(name)
                .unwrap_or_else(|| panic!("{name} is not registered"))
                .unwrap_or_else(|error| panic!("{name} does not parse: {error}"));
            assert_eq!(preset.name, name, "{name} carries a different name");
            assert!(warnings.is_empty(), "{name}: {warnings:?}");
            assert!(
                !preset.output.effects.is_empty() || !preset.input.effects.is_empty(),
                "{name} is empty"
            );
        }
    }

    #[test]
    fn every_builtin_preset_names_a_known_effect() {
        for name in names() {
            let (preset, _) = find(name).unwrap().unwrap();
            for section in [&preset.output, &preset.input] {
                for slot in &section.effects {
                    assert!(
                        pwe_dsp::find(&slot.kind).is_some(),
                        "{name} names the unknown effect '{}'",
                        slot.kind
                    );
                }
            }
        }
    }

    #[test]
    fn every_builtin_parameter_belongs_to_its_effect() {
        let mut reported = Vec::new();
        for name in names() {
            let (preset, _) = find(name).unwrap().unwrap();
            for section in [&preset.output, &preset.input] {
                for slot in &section.effects {
                    let descriptor = pwe_dsp::find(&slot.kind).expect("the effect is registered");
                    for id in slot.params.keys() {
                        if descriptor.specs().iter().all(|spec| spec.id != id) {
                            reported.push(format!("{name}: {}.{id}", slot.kind));
                        }
                    }
                }
            }
        }
        assert!(reported.is_empty(), "{reported:?}");
    }

    #[test]
    fn unknown_names_are_reported() {
        assert!(find("Nope").is_none());
        assert!(names().contains(&"Default"));
    }
}
