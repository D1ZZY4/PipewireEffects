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
