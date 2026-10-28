mod support;

use support::*;

#[test]
fn the_limiter_never_lets_a_sample_past_the_ceiling() {
    let mut effect = LimiterEffect::new();
    effect
        .parameters()
        .set_by_id("threshold", ParamValue::Float(-6.0))
        .unwrap();
    effect.prepare(format());

    let ceiling = db_to_linear(-6.0);

    let mut samples = sine(400.0, 96_000, 1.0);
    process_in_blocks(&mut effect, &mut samples);

    let loudest = peak(&samples);
    assert!(
        loudest <= ceiling + 1e-6,
        "limiter emitted {loudest} with a ceiling of {ceiling}"
    );
}

#[test]
fn the_limiter_leaves_a_quiet_signal_untouched() {
    let mut effect = LimiterEffect::new();
    effect
        .parameters()
        .set_by_id("threshold", ParamValue::Float(-6.0))
        .unwrap();
    effect.prepare(format());

    let mut samples = sine(1_000.0, 9_600, 0.1);
    process_in_blocks(&mut effect, &mut samples);

    let measured = tone_magnitude(settled_tail(&samples), 1_000.0);
    assert!(
        (measured - 0.1).abs() < 1e-3,
        "a signal 20 dB below the ceiling was altered: {measured}"
    );
}

#[test]
fn make_up_gain_lifts_a_quiet_signal_and_is_still_limited() {
    let mut effect = LimiterEffect::new();
    effect
        .parameters()
        .set_by_id("threshold", ParamValue::Float(-1.0))
        .unwrap();
    effect
        .parameters()
        .set_by_id("makeup", ParamValue::Float(20.0))
        .unwrap();
    effect.prepare(format());

    let ceiling = db_to_linear(-1.0);

    let mut samples = sine(1_000.0, 96_000, 0.05);
    process_in_blocks(&mut effect, &mut samples);

    let loudest = peak(&samples);
    assert!(
        loudest <= ceiling + 1e-6,
        "make-up gain pushed the output to {loudest}, above the ceiling {ceiling}"
    );

    let tail_peak = peak(settled_tail(&samples));
    assert!(
        tail_peak > 0.4,
        "make-up gain had little effect, tail peak was only {tail_peak}"
    );
}

#[test]
fn release_lets_the_limiter_recover_after_a_transient() {
    let mut effect = LimiterEffect::new();
    effect
        .parameters()
        .set_by_id("threshold", ParamValue::Float(-12.0))
        .unwrap();
    effect
        .parameters()
        .set_by_id("release", ParamValue::Float(20.0))
        .unwrap();
    effect.prepare(format());

    let mut samples = sine(500.0, 24_000, 1.0);
    samples.extend(sine(500.0, 72_000, 0.05));
    process_in_blocks(&mut effect, &mut samples);

    let quiet_start = 24_000 * CHANNELS;
    let recovered = peak(&samples[quiet_start + 24_000..]);
    let expected = 0.05;
    assert!(
        (recovered - expected).abs() < 0.005,
        "the limiter did not recover, expected {expected} but measured {recovered}"
    );
}

#[test]
fn the_limiter_is_transparent_to_a_linked_stereo_signal_above_the_ceiling() {
    let mut effect = LimiterEffect::new();
    effect
        .parameters()
        .set_by_id("threshold", ParamValue::Float(-3.0))
        .unwrap();
    effect
        .parameters()
        .set_by_id("linked", ParamValue::Bool(true))
        .unwrap();
    effect.prepare(format());

    let frames = 9_600;
    let mut samples = vec![0.0_f32; frames * CHANNELS];
    for frame in 0..frames {
        #[allow(clippy::cast_precision_loss)]
        let phase = 2.0 * PI * 500.0 * frame as f32 / RATE as f32;
        samples[frame * CHANNELS] = phase.sin();
    }
    process_in_blocks(&mut effect, &mut samples);

    assert!(peak(&samples) <= db_to_linear(-3.0) + 1e-6);
    let right_peak = samples
        .chunks_exact(CHANNELS)
        .fold(0.0_f32, |acc, frame| acc.max(frame[1].abs()));
    assert!(right_peak < 1e-9);
}

#[test]
fn the_registry_is_well_formed() {
    let registry = pipewire_effects_dsp::registry();
    assert!(!registry.is_empty(), "the registry must not be empty");

    let mut kinds = Vec::new();
    for def in registry {
        let effect = def.instantiate();

        assert_eq!(
            effect.kind(),
            def.kind,
            "kind() disagrees with the registry"
        );
        assert!(!def.label.is_empty(), "{} has no label", def.kind);
        assert!(!def.summary.is_empty(), "{} has no summary", def.kind);
        assert!(
            !kinds.contains(&def.kind),
            "{} is registered twice",
            def.kind
        );
        kinds.push(def.kind);

        let parameters = effect.parameters();
        assert!(!parameters.is_empty(), "{} exposes no parameters", def.kind);

        let mut ids = Vec::new();
        for (index, spec) in parameters.specs().iter().enumerate() {
            assert!(
                !ids.contains(&spec.id),
                "{} declares `{}` twice",
                def.kind,
                spec.id
            );
            ids.push(spec.id.clone());
            assert!(
                !spec.label.is_empty(),
                "{}::{} has no label",
                def.kind,
                spec.id
            );

            let value = parameters
                .value(index)
                .unwrap_or_else(|| panic!("{}::{} has no value", def.kind, spec.id));
            assert!(
                value.is_finite(),
                "{}::{} defaults to a non-finite value",
                def.kind,
                spec.id
            );
            if let Some((min, max)) = spec.kind.bounds() {
                let raw = value.to_f32();
                assert!(
                    raw >= min && raw <= max,
                    "{}::{} defaults to {raw}, outside {min}..={max}",
                    def.kind,
                    spec.id
                );
            }
            if let Some(count) = spec.kind.option_count() {
                assert!(
                    value.to_index() < count,
                    "{}::{} defaults outside its option list",
                    def.kind,
                    spec.id
                );
            }

            let _ = spec.kind.format_value(value);
            let _ = spec.kind.normalise(value.to_f32());
            let _ = spec.kind.denormalise(0.5);
        }
    }
}

#[test]
fn every_registered_effect_survives_silence_noise_and_a_hostile_signal() {
    for def in pipewire_effects_dsp::registry() {
        for source in ["silence", "noise", "extremes"] {
            let mut effect = def.instantiate();
            effect.prepare(format());

            let mut samples = vec![0.0_f32; 24_000 * CHANNELS];
            match source {
                "noise" => {
                    let mut state = 0x1234_5678_u32;
                    for sample in &mut samples {
                        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        #[allow(clippy::cast_precision_loss)]
                        {
                            *sample = ((state >> 8) as f32 / 8_388_608.0) - 1.0;
                        }
                    }
                }
                "extremes" => {
                    for (index, sample) in samples.iter_mut().enumerate() {
                        *sample = match index % 4 {
                            0 => 1_000.0,
                            1 => -1_000.0,
                            2 => 0.0,
                            _ => 1e-40,
                        };
                    }
                }
                _ => {}
            }

            process_in_blocks(effect.as_mut(), &mut samples);
            assert!(
                samples.iter().all(|s| s.is_finite()),
                "{} produced non-finite output from {source}",
                def.kind
            );
            let loudest = peak(&samples);
            assert!(
                loudest < 10_000.0,
                "{} ran away on {source}: reached {loudest}",
                def.kind
            );
            let _ = rms(&samples);
        }
    }
}
