mod support;

use support::*;

#[test]
fn gain_attenuates_by_the_requested_number_of_decibels() {
    let mut effect = GainEffect::new();
    effect
        .parameters()
        .set_by_id("gain", ParamValue::Float(-6.0))
        .unwrap();
    effect.prepare(format());

    let mut samples = sine(1_000.0, 48_000, 0.5);
    process_in_blocks(&mut effect, &mut samples);

    let measured = tone_magnitude(settled_tail(&samples), 1_000.0);
    let expected = 0.5 * db_to_linear(-6.0);
    assert!(
        (measured - expected).abs() < 1e-3,
        "expected {expected}, measured {measured}"
    );
}

#[test]
fn gain_at_full_scale_is_bit_exact() {
    let mut effect = GainEffect::new();
    effect.prepare(format());

    let original = sine(1_000.0, 4_800, 0.5);
    let mut samples = original.clone();
    process_in_blocks(&mut effect, &mut samples);
    assert_eq!(samples, original, "0 dB must be a true pass-through");
}

#[test]
fn mute_silences_the_output() {
    let mut effect = GainEffect::new();
    effect
        .parameters()
        .set_by_id("mute", ParamValue::Bool(true))
        .unwrap();
    effect.prepare(format());

    let mut samples = sine(1_000.0, 4_800, 0.5);
    process_in_blocks(&mut effect, &mut samples);
    assert!(peak(&samples) < 1e-6);
}

#[test]
fn a_gain_change_is_ramped_rather_than_stepped() {
    let mut effect = GainEffect::new();
    effect
        .parameters()
        .set_by_id("gain", ParamValue::Float(-6.0))
        .unwrap();
    effect.prepare(format());

    let mut samples = vec![0.5_f32; 4_800 * CHANNELS];
    process_in_blocks(&mut effect, &mut samples);

    effect
        .parameters()
        .set_by_id("gain", ParamValue::Float(6.0))
        .unwrap();
    let mut changed = vec![0.5_f32; 4_800 * CHANNELS];
    process_in_blocks(&mut effect, &mut changed);

    let largest_step = changed
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        largest_step < 0.01,
        "gain change produced a step of {largest_step}, which would click"
    );
}

#[test]
fn a_fresh_equaliser_is_transparent() {
    let mut effect = ParametricEqEffect::new();
    effect.prepare(format());

    let original = sine(1_000.0, 4_800, 0.4);
    let mut samples = original.clone();
    process_in_blocks(&mut effect, &mut samples);

    for (processed, reference) in samples.iter().zip(&original) {
        assert!(
            (processed - reference).abs() < 1e-6,
            "a flat equaliser altered the signal"
        );
    }
    assert!(effect.is_transparent());
}

#[test]
fn boosting_a_band_raises_that_frequency_and_leaves_others_alone() {
    let mut effect = ParametricEqEffect::new();
    effect
        .parameters()
        .set_by_id("band4_gain", ParamValue::Float(12.0))
        .unwrap();
    effect.prepare(format());

    let mut samples = sine(1_000.0, 48_000, 0.25);
    process_in_blocks(&mut effect, &mut samples);
    let boosted = tone_magnitude(settled_tail(&samples), 1_000.0);

    let mut reference = sine(1_000.0, 48_000, 0.25);
    let untouched = tone_magnitude(settled_tail(&reference), 1_000.0);
    reference.fill(0.0);

    let gain_db = linear_to_db(boosted / untouched);
    assert!(
        (gain_db - 12.0).abs() < 0.5,
        "expected a 12 dB boost at 1 kHz, measured {gain_db} dB"
    );
    assert!(!effect.is_transparent());
}

#[test]
fn a_band_boost_does_not_affect_a_distant_frequency() {
    let mut effect = ParametricEqEffect::new();
    effect
        .parameters()
        .set_by_id("band4_gain", ParamValue::Float(12.0))
        .unwrap();
    effect
        .parameters()
        .set_by_id("band4_q", ParamValue::Float(2.0))
        .unwrap();
    effect.prepare(format());

    let mut samples = sine(100.0, 48_000, 0.25);
    process_in_blocks(&mut effect, &mut samples);

    let measured = tone_magnitude(settled_tail(&samples), 100.0);
    let gain_db = linear_to_db(measured / 0.25);
    assert!(
        gain_db.abs() < 0.3,
        "a 1 kHz boost leaked {gain_db} dB into 100 Hz"
    );
}

#[test]
fn a_high_pass_band_removes_direct_current() {
    let mut effect = ParametricEqEffect::new();
    effect
        .parameters()
        .set_by_id("band1_type", ParamValue::Enum(3)) // High Pass
        .unwrap();
    effect
        .parameters()
        .set_by_id("band1_frequency", ParamValue::Float(500.0))
        .unwrap();
    effect
        .parameters()
        .set_by_id("band1_enabled", ParamValue::Bool(true))
        .unwrap();
    effect.prepare(format());

    let mut samples = vec![0.5_f32; 48_000 * CHANNELS];
    process_in_blocks(&mut effect, &mut samples);

    let tail = settled_tail(&samples);
    assert!(
        peak(tail) < 0.01,
        "high pass left {} of a DC offset behind",
        peak(tail)
    );
}

#[test]
fn every_band_shape_survives_a_full_sweep_of_its_parameters() {
    for band_type in 0..8 {
        for frequency in [20.0_f32, 1_000.0, 20_000.0] {
            for gain in [-24.0_f32, 24.0] {
                for q in [0.1_f32, 10.0] {
                    let mut effect = ParametricEqEffect::new();
                    let parameters = effect.parameters();
                    parameters
                        .set_by_id("band1_type", ParamValue::Enum(band_type))
                        .unwrap();
                    parameters
                        .set_by_id("band1_frequency", ParamValue::Float(frequency))
                        .unwrap();
                    parameters
                        .set_by_id("band1_gain", ParamValue::Float(gain))
                        .unwrap();
                    parameters
                        .set_by_id("band1_q", ParamValue::Float(q))
                        .unwrap();
                    effect.prepare(format());

                    let mut samples = sine(1_000.0, 9_600, 0.5);
                    process_in_blocks(&mut effect, &mut samples);
                    assert!(
                        samples.iter().all(|s| s.is_finite()),
                        "shape {band_type} f={frequency} gain={gain} q={q} produced non-finite output"
                    );
                    assert!(
                        peak(&samples) < 100.0,
                        "shape {band_type} f={frequency} gain={gain} q={q} blew up"
                    );
                }
            }
        }
    }
}
