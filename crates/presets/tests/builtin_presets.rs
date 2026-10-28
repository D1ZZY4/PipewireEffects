use std::collections::BTreeSet;

use pipewire_effects_core::find_def;
use pipewire_effects_presets::PresetStore;

#[test]
fn every_builtin_preset_parses() {
    let builtins = PresetStore::builtin().expect("built-in presets must parse");
    assert!(!builtins.is_empty(), "there are no built-in presets");
}

#[test]
fn every_builtin_preset_has_a_name_and_description() {
    for preset in PresetStore::builtin().unwrap() {
        assert!(!preset.name.is_empty(), "a built-in preset has no name");
        assert!(
            !preset.description.is_empty(),
            "`{}` has no description, which would show as an empty row",
            preset.name
        );
        assert!(
            preset.file_stem().is_ok(),
            "`{}` does not produce a usable file name",
            preset.name
        );
    }
}

#[test]
fn builtin_preset_names_are_unique() {
    let mut seen = BTreeSet::new();
    for preset in PresetStore::builtin().unwrap() {
        assert!(
            seen.insert(preset.name.clone()),
            "`{}` appears twice",
            preset.name
        );
    }
}

#[test]
fn every_builtin_preset_only_references_effects_that_exist() {
    let registry = pipewire_effects_dsp::registry();
    for preset in PresetStore::builtin().unwrap() {
        for kind in preset.effect_kinds() {
            assert!(
                find_def(registry, kind).is_some(),
                "preset `{}` references the unknown effect `{kind}`",
                preset.name
            );
        }
    }
}

#[test]
fn every_builtin_preset_only_sets_parameters_that_exist() {
    let registry = pipewire_effects_dsp::registry();
    for preset in PresetStore::builtin().unwrap() {
        for entry in &preset.output.effects {
            let definition = find_def(registry, &entry.kind)
                .unwrap_or_else(|| panic!("preset `{}` uses an unknown effect", preset.name));

            let probe = definition.instantiate();
            let parameters = probe.parameters();

            for id in entry.parameters.keys() {
                assert!(
                    parameters.index_of(id).is_some(),
                    "preset `{}` sets `{}::{id}`, which does not exist",
                    preset.name,
                    entry.kind
                );
            }
        }
    }
}

#[test]
fn builtin_preset_values_are_in_range_for_their_parameters() {
    let registry = pipewire_effects_dsp::registry();
    for preset in PresetStore::builtin().unwrap() {
        for entry in &preset.output.effects {
            let definition = find_def(registry, &entry.kind).unwrap();
            let probe = definition.instantiate();
            let parameters = probe.parameters();

            for (id, value) in &entry.parameters {
                let index = parameters.index_of(id).unwrap();
                let spec = parameters.spec(index).unwrap();
                if let Some((min, max)) = spec.kind.bounds() {
                    let raw = value.to_value().to_f32();
                    assert!(
                        raw >= min && raw <= max,
                        "preset `{}` sets `{}::{id}` to {raw}, outside {min}..={max}",
                        preset.name,
                        entry.kind
                    );
                }
                if let Some(count) = spec.kind.option_count() {
                    let index_value = value.to_value().to_index();
                    assert!(
                        index_value < count,
                        "preset `{}` sets `{}::{id}` to an option index of {index_value}, but there are only {count} options",
                        preset.name,
                        entry.kind
                    );
                }
            }
        }
    }
}

#[test]
fn builtin_presets_apply_cleanly_to_a_controller() {
    let controller = pipewire_effects_core::ChainController::new(pipewire_effects_dsp::registry());

    for preset in PresetStore::builtin().unwrap() {
        let report = controller
            .apply_recipe(&preset.to_recipe())
            .unwrap_or_else(|error| panic!("preset `{}` failed to apply: {error}", preset.name));

        assert!(
            report.is_complete(),
            "preset `{}` was only partly applied: {report:?}",
            preset.name
        );

        let expected = preset.output.effects.len();
        assert_eq!(
            controller.len(),
            expected,
            "preset `{}` expected {expected} effects in the chain",
            preset.name
        );
    }
}

#[test]
fn builtin_presets_survive_a_json_round_trip() {
    for preset in PresetStore::builtin().unwrap() {
        let json = serde_json::to_string(&preset).unwrap();
        let parsed: pipewire_effects_presets::Preset = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, preset, "`{}` did not round trip", preset.name);
    }
}
