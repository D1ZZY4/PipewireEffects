use super::*;
use crate::biquad::FilterKind;

fn kind_index(kind: FilterKind) -> u16 {
    FilterKind::ALL
        .iter()
        .position(|value| *value == kind)
        .expect("the filter kind is published") as u16
}

fn channel_peak(buffer: &AudioBuffer, channel: usize) -> f32 {
    buffer
        .channel(channel)
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
}

fn tone(frequency: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn band_position(frequency: f32) -> usize {
    BAND_FREQUENCIES
        .iter()
        .position(|value| (*value - frequency).abs() < 1.0)
        .expect("the band should exist")
}

#[test]
fn the_ten_legacy_bands_keep_their_frequencies() {
    let legacy = [
        (31.25, "band1_gain"),
        (62.5, "band2_gain"),
        (125.0, "band3_gain"),
        (250.0, "band4_gain"),
        (500.0, "band5_gain"),
        (1_000.0, "band6_gain"),
        (2_000.0, "band7_gain"),
        (4_000.0, "band8_gain"),
        (8_000.0, "band9_gain"),
        (16_000.0, "band10_gain"),
    ];
    for (frequency, id) in legacy {
        let position = band_position(frequency);
        assert_eq!(BAND_IDS[position], id, "{frequency} Hz");
    }
}

#[test]
fn the_bands_are_in_ascending_order_and_unique() {
    for pair in BAND_FREQUENCIES.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?}");
    }
    let mut ids: Vec<&str> = BAND_IDS.to_vec();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), BAND_COUNT);
}

#[test]
fn a_flat_equaliser_is_transparent() {
    let params = ParamSet::new(specs());
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(1_000.0, 1024);
    let reference = buffer.channel(0).to_vec();
    effect.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
        assert!(
            (processed - original).abs() < 1.0e-3,
            "{processed} vs {original}"
        );
    }
}

#[test]
fn boosting_one_band_raises_that_frequency() {
    let params = ParamSet::new(specs());
    params.set_f32("band6_gain", 8.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    for _ in 0..64 {
        let mut buffer = tone(1_000.0, 1024);
        effect.process(&mut buffer);
        peak = peak.max(buffer.peak());
    }
    let expected = math::db_to_gain(8.0);
    assert!(
        (peak - expected).abs() < 0.3,
        "peak {peak} expected {expected}"
    );
}

#[test]
fn a_new_band_boosts_its_own_frequency() {
    let params = ParamSet::new(specs());
    let position = band_position(2_500.0);
    params.set_f32(BAND_IDS[position], 9.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    for _ in 0..64 {
        let mut buffer = tone(2_500.0, 1024);
        effect.process(&mut buffer);
        peak = peak.max(buffer.peak());
    }
    assert!((peak - math::db_to_gain(9.0)).abs() < 0.3, "peak {peak}");
}

#[test]
fn boosting_one_band_does_not_leak_into_another() {
    let params = ParamSet::new(specs());
    params.set_f32("band8_gain", 12.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    for _ in 0..64 {
        let mut buffer = tone(125.0, 1024);
        effect.process(&mut buffer);
        peak = peak.max(buffer.peak());
    }
    assert!(peak < 1.1, "leakage {peak}");
}

#[test]
fn analytic_response_matches_the_measured_response() {
    let params = ParamSet::new(specs());
    params.set_f32("band4_gain", 6.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    for _ in 0..64 {
        let mut buffer = tone(250.0, 1024);
        effect.process(&mut buffer);
        peak = peak.max(buffer.peak());
    }
    let response = ParamSet::new(specs());
    response
        .set_f32(BAND_IDS[band_position(250.0)], 6.0)
        .unwrap();
    let predicted = math::db_to_gain(response_db(48_000.0, &response, false, 250.0));
    assert!(
        (peak - predicted).abs() < 0.05,
        "measured {peak} predicted {predicted}"
    );
}

#[test]
fn the_bands_are_grouped_into_three_editor_sections() {
    let mut groups: Vec<&str> = Vec::new();
    for spec in specs()
        .iter()
        .filter(|spec| spec.id.starts_with("band") && spec.id.ends_with("_gain"))
    {
        if !groups.contains(&spec.group) {
            groups.push(spec.group);
        }
    }
    assert_eq!(groups, ["Lows", "Mids", "Highs"]);
}

#[test]
fn output_gain_is_applied() {
    let params = ParamSet::new(specs());
    params.set_f32("output_gain", -6.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(1_000.0, 1024);
    effect.process(&mut buffer);
    assert!((buffer.peak() - math::db_to_gain(-6.0)).abs() < 0.01);
}

#[test]
fn every_band_publishes_type_frequency_and_q() {
    let params = ParamSet::new(specs());
    for (band, frequency) in BAND_FREQUENCIES.iter().enumerate() {
        for id in [
            params::BAND_TYPE_IDS[band],
            params::BAND_FREQUENCY_IDS[band],
            params::BAND_Q_IDS[band],
        ] {
            assert!(params.spec(id).is_some(), "{id} is missing");
        }
        assert_eq!(
            params.get_f32(params::BAND_TYPE_IDS[band]),
            Some(kind_index(FilterKind::Peaking) as f32)
        );
        assert_eq!(
            params.get_f32(params::BAND_FREQUENCY_IDS[band]),
            Some(*frequency)
        );
        assert_eq!(params.get_f32(params::BAND_Q_IDS[band]), Some(1.414));
        assert_eq!(
            params.get_f32(params::RIGHT_BAND_TYPE_IDS[band]),
            Some(kind_index(FilterKind::Peaking) as f32)
        );
        assert_eq!(
            params.get_f32(params::RIGHT_BAND_FREQUENCY_IDS[band]),
            Some(*frequency)
        );
    }
}

#[test]
fn the_band_type_changes_the_response() {
    let params = ParamSet::new(specs());
    let position = band_position(250.0);
    params
        .set_choice(
            params::BAND_TYPE_IDS[position],
            kind_index(FilterKind::LowPass),
        )
        .unwrap();
    let response = response_db(48_000.0, &params, false, 4_000.0);
    assert!(response < -20.0, "response {response}");
    let passband = response_db(48_000.0, &params, false, 50.0);
    assert!(passband.abs() < 0.5, "passband {passband}");
}

#[test]
fn a_band_frequency_moves_the_filter() {
    let params = ParamSet::new(specs());
    let position = band_position(1_000.0);
    params
        .set_f32(params::BAND_FREQUENCY_IDS[position], 250.0)
        .unwrap();
    params.set_f32(BAND_IDS[position], 8.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    for _ in 0..64 {
        let mut buffer = tone(250.0, 1024);
        effect.process(&mut buffer);
        peak = peak.max(buffer.peak());
    }
    assert!((peak - math::db_to_gain(8.0)).abs() < 0.3, "peak {peak}");
}

#[test]
fn q_sets_the_band_width() {
    let wide = ParamSet::new(specs());
    wide.set_f32(params::BAND_Q_IDS[band_position(1_000.0)], 0.5)
        .unwrap();
    wide.set_f32(BAND_IDS[band_position(1_000.0)], 12.0)
        .unwrap();
    let narrow = ParamSet::new(specs());
    narrow
        .set_f32(params::BAND_Q_IDS[band_position(1_000.0)], 5.0)
        .unwrap();
    narrow
        .set_f32(BAND_IDS[band_position(1_000.0)], 12.0)
        .unwrap();
    let wide_neighbour = response_db(48_000.0, &wide, false, 2_000.0);
    let narrow_neighbour = response_db(48_000.0, &narrow, false, 2_000.0);
    assert!(wide_neighbour > narrow_neighbour + 3.0);
}

#[test]
fn split_channels_route_each_band_independently() {
    let params = ParamSet::new(specs());
    params.set_choice("channel_mode", 1).unwrap();
    let position = band_position(1_000.0);
    params.set_f32(BAND_IDS[position], 12.0).unwrap();
    params.set_f32(RIGHT_BAND_IDS[position], -12.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut last = tone(1_000.0, 2_048);
    for _ in 0..32 {
        last = tone(1_000.0, 2_048);
        effect.process(&mut last);
    }
    let left = channel_peak(&last, 0);
    let right = channel_peak(&last, 1);
    assert!(left > 0.9, "left {left}");
    assert!(right < left * 0.25, "left {left} right {right}");
}

#[test]
fn stereo_mode_uses_the_shared_bands_on_both_channels() {
    let params = ParamSet::new(specs());
    let position = band_position(1_000.0);
    params.set_f32(BAND_IDS[position], 6.0).unwrap();
    let mut effect = Equalizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut last = tone(1_000.0, 2_048);
    for _ in 0..32 {
        last = tone(1_000.0, 2_048);
        effect.process(&mut last);
    }
    assert!((channel_peak(&last, 0) - channel_peak(&last, 1)).abs() < 1.0e-3);
}

#[test]
fn the_legacy_bandwidth_remains_the_default_q() {
    let params = ParamSet::new(specs());
    params.set_f32("bandwidth", 3.0).unwrap();
    params
        .set_f32(BAND_IDS[band_position(1_000.0)], 8.0)
        .unwrap();
    let with_bandwidth = response_db(48_000.0, &params, false, 2_000.0);
    params
        .set_f32(params::BAND_Q_IDS[band_position(1_000.0)], 3.0)
        .unwrap();
    let explicit = response_db(48_000.0, &params, false, 2_000.0);
    assert!((with_bandwidth - explicit).abs() < 1.0e-4);
}
