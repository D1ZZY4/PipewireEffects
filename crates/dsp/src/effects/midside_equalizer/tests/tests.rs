use super::*;
use crate::biquad::FilterKind;

fn kind_index(kind: FilterKind) -> u16 {
    FilterKind::ALL
        .iter()
        .position(|value| *value == kind)
        .expect("the filter kind is published") as u16
}

fn settings(pairs: &[(&str, f32)]) -> ParamSet {
    let params = ParamSet::new(specs());
    for (key, value) in pairs {
        params.set_f32(key, *value).unwrap();
    }
    params
}

fn settled_with(params: ParamSet, left: f32, right: f32, frequency: f32) -> AudioBuffer {
    let mut effect = MidSideEqualizer::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut peak = 0.0f32;
    let mut last = tone(left, right, frequency, 2_048);
    for _ in 0..32 {
        last = tone(left, right, frequency, 2_048);
        effect.process(&mut last);
        peak = peak.max(last.peak());
    }
    assert!(peak.is_finite());
    last
}

fn tone(left: f32, right: f32, frequency: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for index in 0..frames {
        let value = (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
        buffer.channel_mut(0)[index] = value * left;
        buffer.channel_mut(1)[index] = value * right;
    }
    buffer
}

fn settled(pairs: &[(&str, f32)], left: f32, right: f32, frequency: f32) -> AudioBuffer {
    settled_with(settings(pairs), left, right, frequency)
}

#[test]
fn a_flat_curve_is_transparent() {
    let processed = settled(&[], 0.5, -0.5, 1_000.0);
    let reference = tone(0.5, -0.5, 1_000.0, 2_048);
    for channel in 0..2 {
        for (processed, original) in processed
            .channel(channel)
            .iter()
            .zip(reference.channel(channel))
        {
            assert!(
                (processed - original).abs() < 1.0e-3,
                "{processed} {original}"
            );
        }
    }
}

#[test]
fn a_mid_band_boost_changes_a_mono_signal() {
    let processed = settled(&[("mid_band6_gain", 8.0)], 0.4, 0.4, 1_000.0);
    assert!(processed.peak() > 0.4 + 0.2, "{}", processed.peak());
}

#[test]
fn a_side_band_boost_leaves_a_mono_signal_alone() {
    let processed = settled(&[("side_band6_gain", 12.0)], 0.4, 0.4, 1_000.0);
    assert!(
        (processed.peak() - 0.4).abs() < 0.05,
        "{}",
        processed.peak()
    );
}

#[test]
fn the_side_gain_widens_and_narrows_the_image() {
    let wide = settled(&[("side_gain", 6.0)], 0.3, -0.3, 500.0);
    let narrow = settled(&[("side_gain", -6.0)], 0.3, -0.3, 500.0);
    assert!(wide.peak() > narrow.peak() * 1.5);
}

#[test]
fn the_output_gain_is_applied() {
    let processed = settled(&[("output_gain", -6.0)], 0.4, -0.4, 800.0);
    assert!((processed.peak() - 0.4 * math::db_to_gain(-6.0)).abs() < 0.02);
}

#[test]
fn every_mid_and_side_band_publishes_type_frequency_and_q() {
    let params = ParamSet::new(specs());
    for band in 0..BAND_COUNT {
        for id in [
            MID_TYPE_IDS[band],
            MID_FREQUENCY_IDS[band],
            MID_Q_IDS[band],
            SIDE_TYPE_IDS[band],
            SIDE_FREQUENCY_IDS[band],
            SIDE_Q_IDS[band],
        ] {
            assert!(params.spec(id).is_some(), "{id} is missing");
        }
        assert_eq!(
            params.get_f32(MID_TYPE_IDS[band]),
            Some(kind_index(FilterKind::Peaking) as f32)
        );
        assert_eq!(
            params.get_f32(MID_FREQUENCY_IDS[band]),
            Some(super::super::equalizer::params::BAND_FREQUENCIES[band])
        );
        assert_eq!(
            params.get_f32(SIDE_TYPE_IDS[band]),
            Some(kind_index(FilterKind::Peaking) as f32)
        );
        assert_eq!(params.get_f32(SIDE_Q_IDS[band]), Some(1.414));
    }
}

#[test]
fn a_mid_band_frequency_moves_the_filter() {
    let params = settings(&[]);
    let position = 17;
    params.set_f32(MID_FREQUENCY_IDS[position], 250.0).unwrap();
    params.set_f32(MID_GAIN_IDS[position], 8.0).unwrap();
    let processed = settled_with(params, 0.4, 0.4, 250.0);
    assert!(processed.peak() > 0.7, "{}", processed.peak());
}

#[test]
fn a_side_band_frequency_moves_the_filter() {
    let params = settings(&[]);
    let position = 17;
    params.set_f32(SIDE_FREQUENCY_IDS[position], 250.0).unwrap();
    params.set_f32(SIDE_GAIN_IDS[position], 12.0).unwrap();
    let near = settled_with(params.clone(), 0.4, -0.4, 250.0);
    let far = settled_with(params, 0.4, -0.4, 1_000.0);
    assert!(near.peak() > far.peak() * 2.0);
}

#[test]
fn a_band_type_changes_the_mid_response() {
    let params = settings(&[]);
    let position = 11;
    params
        .set_choice(MID_TYPE_IDS[position], kind_index(FilterKind::LowPass))
        .unwrap();
    let processed = settled_with(params, 0.4, 0.4, 4_000.0);
    assert!(processed.peak() < 0.1, "{}", processed.peak());
}

#[test]
fn q_changes_the_mid_band_width() {
    let wide = settings(&[]);
    wide.set_f32(MID_Q_IDS[17], 0.5).unwrap();
    wide.set_f32(MID_GAIN_IDS[17], 12.0).unwrap();
    let narrow = settings(&[]);
    narrow.set_f32(MID_Q_IDS[17], 5.0).unwrap();
    narrow.set_f32(MID_GAIN_IDS[17], 12.0).unwrap();
    let wide_peak = settled_with(wide, 0.4, 0.4, 2_000.0).peak();
    let narrow_peak = settled_with(narrow, 0.4, 0.4, 2_000.0).peak();
    assert!(wide_peak > narrow_peak * 1.2);
}

#[test]
fn the_legacy_bandwidth_remains_the_default_q() {
    let bandwidth = settings(&[("bandwidth", 3.0)]);
    let explicit = settings(&[("mid_band6_q", 3.0)]);
    let legacy = BandKnobs::new(
        &bandwidth,
        MID_GAIN_IDS[17],
        MID_TYPE_IDS[17],
        MID_FREQUENCY_IDS[17],
        MID_Q_IDS[17],
    )
    .coefficients(48_000.0, 3.0);
    let named = BandKnobs::new(
        &explicit,
        MID_GAIN_IDS[17],
        MID_TYPE_IDS[17],
        MID_FREQUENCY_IDS[17],
        MID_Q_IDS[17],
    )
    .coefficients(48_000.0, 1.414);
    assert_eq!(legacy, named);
}
