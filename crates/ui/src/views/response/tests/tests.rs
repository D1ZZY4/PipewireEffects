use super::*;

fn equalizer() -> ParamSet {
    ParamSet::new(pwe_dsp::effects::equalizer_specs())
}

#[test]
fn the_log_axis_is_monotonic() {
    let low = log_position(20.0);
    let mid = log_position(632.0);
    let high = log_position(20_000.0);
    assert!(low < mid && mid < high);
    assert!(low.abs() < 1.0e-6);
    assert!((high - 1.0).abs() < 1.0e-6);
}

#[test]
fn the_axis_round_trips() {
    for position in [0.0f32, 0.25, 0.5, 1.0] {
        assert!((log_position(hz_at(position)) - position).abs() < 1.0e-4);
    }
}

#[test]
fn the_parameter_ids_follow_the_gain_id() {
    assert_eq!(field_id("band6_gain", "type"), "band6_type");
    assert_eq!(field_id("band6_gain", "frequency"), "band6_frequency");
    assert_eq!(field_id("band6_gain", "q"), "band6_q");
    assert_eq!(channel_id("band6_gain", "gain", false), "band6_gain");
    assert_eq!(channel_id("band6_gain", "gain", true), "right_band6_gain");
}

#[test]
fn the_split_flag_follows_the_channel_choice() {
    let params = equalizer();
    assert!(!split_channels(&params));
    params.set_choice("channel_mode", 1).unwrap();
    assert!(split_channels(&params));
}

#[test]
fn band_frequencies_and_gains_come_from_the_parameters() {
    let params = equalizer();
    let band = BAND_IDS.iter().position(|id| *id == "band6_gain").unwrap();
    params.set_f32("band6_frequency", 250.0).unwrap();
    params.set_f32("band6_gain", 7.5).unwrap();
    params.set_f32("right_band6_frequency", 4_000.0).unwrap();
    assert_eq!(band_frequency(&params, band, false), 250.0);
    assert_eq!(band_frequency(&params, band, true), 4_000.0);
    assert_eq!(band_gain(&params, band, false), 7.5);
}

#[test]
fn the_curve_reflects_frequency_type_and_gain() {
    let params = equalizer();
    assert!(response_db(48_000.0, &params, false, 1_000.0).abs() < 1.0e-3);
    params.set_f32("band6_gain", 6.0).unwrap();
    assert!((response_db(48_000.0, &params, false, 1_000.0) - 6.0).abs() < 0.1);
    params.set_f32("band6_frequency", 250.0).unwrap();
    assert!(response_db(48_000.0, &params, false, 1_000.0).abs() < 0.5);
    params.set_choice("band6_type", 0).unwrap();
    assert!(response_db(48_000.0, &params, false, 1_000.0) < -10.0);
}
