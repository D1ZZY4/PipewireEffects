use pwe_core::ParamKind;

use super::*;

#[test]
fn the_translation_template_carries_every_label_the_registry_publishes() {
    const POT: &str = include_str!("../../../../../../po/pipewire-effects.pot");
    let joined = POT.replace("\"\n\"", "");
    let mut missing: Vec<&str> = Vec::new();
    for descriptor in EFFECTS {
        for text in [descriptor.label, descriptor.summary] {
            if !joined.contains(&format!("msgid \"{text}\"")) {
                missing.push(text);
            }
        }
        for spec in descriptor.specs() {
            for text in [spec.label, spec.group] {
                if !joined.contains(&format!("msgid \"{text}\"")) {
                    missing.push(text);
                }
            }
            if let ParamKind::Choice { labels, .. } = spec.kind {
                for label in labels {
                    if !joined.contains(&format!("msgid \"{label}\"")) {
                        missing.push(label);
                    }
                }
            }
        }
    }
    assert!(missing.is_empty(), "not in the template: {missing:?}");
}

#[test]
fn every_descriptor_has_unique_identity_and_parameters() {
    let mut ids: Vec<&str> = Vec::new();
    let mut labels: Vec<&str> = Vec::new();
    for descriptor in EFFECTS {
        assert!(
            !ids.contains(&descriptor.id),
            "{} is listed twice",
            descriptor.id
        );
        assert!(
            !labels.contains(&descriptor.label),
            "{} shares its label",
            descriptor.id
        );
        assert!(
            !descriptor.summary.is_empty(),
            "{} has no summary",
            descriptor.id
        );
        ids.push(descriptor.id);
        labels.push(descriptor.label);
    }
}

#[test]
fn every_descriptor_publishes_parameters_and_builds() {
    for descriptor in EFFECTS {
        let specs = descriptor.specs();
        assert!(
            !specs.is_empty(),
            "{} publishes no parameters",
            descriptor.id
        );
        let params = ParamSet::new(specs);
        let mut effect = (descriptor.build)(params.clone());
        assert_eq!(effect.id(), descriptor.id);
        effect.prepare(pwe_core::AudioFormat::new(48_000, 2));
        effect.reset();
    }
}

#[test]
fn every_parameter_identifier_is_unique_within_an_effect() {
    for descriptor in EFFECTS {
        let mut seen: Vec<&str> = Vec::new();
        for spec in descriptor.specs() {
            assert!(
                !seen.contains(&spec.id),
                "{} lists {} twice",
                descriptor.id,
                spec.id
            );
            seen.push(spec.id);
        }
    }
}

#[test]
fn an_unknown_identifier_is_not_instantiated() {
    assert!(find("nonesuch").is_none());
    assert!(instantiate("nonesuch").is_none());
    assert!(rebuild("nonesuch", ParamSet::new(Vec::new())).is_none());
}
