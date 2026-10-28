use super::*;
use pipewire_effects_core::ParamUnit;

fn float(id: &str) -> ParamSpec {
    ParamSpec::float(id, id, -24.0, 24.0, 0.0, 0.1, ParamUnit::Decibel)
}

fn boolean(id: &str) -> ParamSpec {
    ParamSpec::boolean(id, id, true)
}

fn enumeration(id: &str) -> ParamSpec {
    ParamSpec::enumerate(id, id, &["a", "b"], 0)
}

fn eq_specs(bands: usize) -> Vec<ParamSpec> {
    let mut specs = vec![float("preamp")];
    for band in 1..=bands {
        let prefix = format!("band{band}_");
        specs.push(boolean(&format!("{prefix}enabled")));
        specs.push(enumeration(&format!("{prefix}type")));
        specs.push(float(&format!("{prefix}frequency")));
        specs.push(float(&format!("{prefix}gain")));
        specs.push(float(&format!("{prefix}q")));
    }
    specs
}

#[test]
fn headings_split_a_trailing_number() {
    assert_eq!(heading_for("band3"), "Band 3");
    assert_eq!(heading_for("preamp"), "Preamp");
    assert_eq!(heading_for("band12"), "Band 12");
}

#[test]
fn parameters_sharing_a_prefix_are_grouped() {
    let specs = vec![float("band1_gain"), float("band1_q"), float("band2_gain")];
    let sections = group_parameters(&specs);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].title, "Band 1");
    assert_eq!(sections[0].indices, vec![0, 1]);
    assert_eq!(sections[1].title, "Band 2");
}

#[test]
fn one_prefix_across_every_parameter_gets_a_generic_title() {
    let specs = vec![float("gain"), boolean("gain_enabled"), float("gain_trim")];
    let sections = group_parameters(&specs);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].title, "Parameters");
}

#[test]
fn bands_are_detected_from_the_identifier_convention() {
    let specs = eq_specs(8);
    let bands = detect_bands(&specs).expect("eight bands should be detected");
    assert_eq!(bands.len(), 8);
    assert_eq!(bands[0].enabled, 1);
    assert_eq!(bands[0].shape, 2);
    assert_eq!(bands[0].frequency, 3);
    assert_eq!(bands[0].gain, 4);
    assert_eq!(bands[0].quality, 5);
    assert_eq!(preamp_index(&specs), Some(0));
}

#[test]
fn a_single_band_is_not_treated_as_an_equaliser() {
    assert!(detect_bands(&eq_specs(1)).is_none());
}

#[test]
fn an_incomplete_band_stops_detection() {
    let mut specs = eq_specs(2);
    specs.truncate(specs.len() - 1);
    assert!(detect_bands(&specs).is_none());
}

#[test]
fn unrelated_parameters_are_not_a_band_effect() {
    let specs = vec![float("gain"), boolean("mute")];
    assert!(detect_bands(&specs).is_none());
}

#[test]
fn every_parameter_lands_in_exactly_one_section() {
    let specs = eq_specs(4);
    let mut seen: Vec<usize> = group_parameters(&specs)
        .into_iter()
        .flat_map(|section| section.indices)
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..specs.len()).collect::<Vec<_>>());
}
