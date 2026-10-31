use super::*;

fn equalizer() -> ParamSet {
    ParamSet::new(pwe_dsp::effects::equalizer_specs())
}

fn band_value(params: &ParamSet, band: usize, field: &str) -> f32 {
    params
        .get_f32(&field_id(BAND_IDS[band], field))
        .unwrap_or(f32::NAN)
}

#[test]
fn a_preamp_sets_the_output_gain() {
    let params = equalizer();
    let report = apply(&params, "Preamp: -6.0 dB\n").unwrap();
    assert_eq!(params.get_f32("output_gain"), Some(-6.0));
    assert_eq!(report.applied, 1);
    assert!(report.warnings.is_empty());
}

#[test]
fn a_graphic_eq_sets_band_frequency_gain_and_type() {
    let params = equalizer();
    let report = apply(&params, "GraphicEQ: 100 -3; 1000 6; 10000 -12").unwrap();
    assert_eq!(band_value(&params, 0, "frequency"), 100.0);
    assert_eq!(band_value(&params, 0, "gain"), -3.0);
    assert_eq!(band_value(&params, 1, "frequency"), 1_000.0);
    assert_eq!(band_value(&params, 1, "gain"), 6.0);
    assert_eq!(band_value(&params, 2, "gain"), -12.0);
    assert_eq!(params.get_f32(&field_id(BAND_IDS[0], "type")), Some(5.0));
    assert_eq!(band_value(&params, 3, "gain"), 0.0);
    assert_eq!(report.applied, 1);
}

#[test]
fn a_graphic_eq_with_more_points_than_bands_is_resampled() {
    let params = equalizer();
    let mut text = String::from("GraphicEQ:");
    for step in 0..40 {
        let frequency = 20.0 * 1.15f32.powi(step);
        text.push_str(&format!(" {frequency:.0} -2;"));
    }
    let report = apply(&params, &text).unwrap();
    assert_eq!(report.warnings.len(), 1);
    for band in 0..BAND_COUNT {
        assert!(band_value(&params, band, "gain").is_finite());
    }
    assert!(band_value(&params, 0, "gain") < 0.0);
}

#[test]
fn a_filter_sets_type_frequency_gain_and_q() {
    let params = equalizer();
    let report = apply(&params, "Filter: ON PK Fc 1000 Hz Gain 6 dB Q 2.5").unwrap();
    assert_eq!(band_value(&params, 0, "frequency"), 1_000.0);
    assert_eq!(band_value(&params, 0, "gain"), 6.0);
    assert_eq!(band_value(&params, 0, "q"), 2.5);
    assert_eq!(params.get_f32(&field_id(BAND_IDS[0], "type")), Some(5.0));
    assert_eq!(report.applied, 1);
}

#[test]
fn filter_types_map_onto_the_filter_kinds() {
    let params = equalizer();
    apply(
        &params,
        "Filter: ON LP Fc 500 Hz\nFilter: ON HS Fc 4000 Hz Gain -4 dB",
    )
    .unwrap();
    assert_eq!(params.get_f32(&field_id(BAND_IDS[0], "type")), Some(0.0));
    assert_eq!(params.get_f32(&field_id(BAND_IDS[1], "type")), Some(7.0));
}

#[test]
fn an_off_filter_is_ignored() {
    let params = equalizer();
    let report = apply(&params, "Filter: OFF PK Fc 1000 Hz Gain 12 dB").unwrap();
    assert_eq!(report.applied, 0);
    assert_eq!(band_value(&params, 0, "gain"), 0.0);
}

#[test]
fn comments_metadata_and_blank_lines_are_ignored() {
    let params = equalizer();
    let report = apply(
        &params,
        "# Equalizer APO\n\nDevice: Speakers\nChannel: L\nPreamp: -2 dB\n",
    )
    .unwrap();
    assert_eq!(params.get_f32("output_gain"), Some(-2.0));
    assert_eq!(report.applied, 1);
}

#[test]
fn an_unknown_directive_is_reported_with_its_line() {
    let params = equalizer();
    let error = apply(&params, "Preamp: 0 dB\nMirror: 1\n").unwrap_err();
    assert!(error.to_string().contains("line 2"), "{error}");
}

#[test]
fn a_malformed_graphic_eq_point_is_reported() {
    let params = equalizer();
    let error = apply(&params, "GraphicEQ: 100\n").unwrap_err();
    assert!(error.to_string().contains("line 1"), "{error}");
}

#[test]
fn an_unknown_filter_type_is_reported() {
    let params = equalizer();
    let error = apply(&params, "Filter: ON XYZ Fc 1000 Hz\n").unwrap_err();
    assert!(error.to_string().contains("line 1"), "{error}");
}

#[test]
fn a_filter_without_a_frequency_is_reported() {
    let params = equalizer();
    let error = apply(&params, "Filter: ON PK Gain 3 dB\n").unwrap_err();
    assert!(error.to_string().contains("line 1"), "{error}");
}

#[test]
fn more_than_thirty_filters_are_reported() {
    let params = equalizer();
    let mut text = String::new();
    for step in 0..31 {
        text.push_str(&format!("Filter: ON PK Fc {} Hz Gain 1 dB\n", 100 + step));
    }
    let report = apply(&params, &text).unwrap();
    assert_eq!(report.applied, BAND_COUNT);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn an_import_replaces_the_previous_curve() {
    let params = equalizer();
    params.set_f32("band1_gain", 9.0).unwrap();
    params.set_f32("output_gain", -3.0).unwrap();
    apply(&params, "Filter: ON PK Fc 1000 Hz Gain 3 dB").unwrap();
    assert_eq!(band_value(&params, 0, "frequency"), 1_000.0);
    assert_eq!(params.get_f32("band1_gain"), Some(0.0));
    assert_eq!(params.get_f32("output_gain"), Some(0.0));
}

#[test]
fn a_malformed_file_leaves_the_parameters_unchanged() {
    let params = equalizer();
    params.set_f32("band6_gain", 5.0).unwrap();
    params.set_f32("output_gain", -3.0).unwrap();
    let error = apply(&params, "Preamp: 2 dB\nGraphicEQ: 100\n").unwrap_err();
    assert!(error.to_string().contains("line 2"), "{error}");
    assert_eq!(params.get_f32("band6_gain"), Some(5.0));
    assert_eq!(params.get_f32("output_gain"), Some(-3.0));
}
