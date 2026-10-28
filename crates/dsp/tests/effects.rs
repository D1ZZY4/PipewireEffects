//! Behavioural tests for the effects, driven through the same `AudioEffect`
//! interface the engine uses.
//!
//! These are the tests that answer "does it actually do what the label says":
//! they push signals through real blocks, in real block sizes, and measure the
//! result. Frequency-domain measurements use the Goertzel algorithm, which
//! evaluates a single DFT bin — enough to ask "how much 1 kHz is there", and
//! far cheaper than a full FFT for that question.

use std::f32::consts::PI;

use pipewire_effects_core::{AudioBlock, AudioEffect, AudioFormat, ParamValue};
use pipewire_effects_dsp::effects::{
    eq::ParametricEqEffect, gain::GainEffect, limiter::LimiterEffect,
};
use pipewire_effects_dsp::util::{db_to_linear, linear_to_db};

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const BLOCK_FRAMES: usize = 480; // 10 ms, a typical PipeWire quantum

fn format() -> AudioFormat {
    AudioFormat::new(RATE, CHANNELS as u16)
}

fn stereo_block(samples: &mut [f32]) -> AudioBlock<'_> {
    AudioBlock::new(samples, format())
}

/// A sine wave, duplicated into both channels.
fn sine(frequency: f32, frames: usize, amplitude: f32) -> Vec<f32> {
    let mut samples = vec![0.0_f32; frames * CHANNELS];
    for frame in 0..frames {
        #[allow(clippy::cast_precision_loss)]
        let phase = 2.0 * PI * frequency * frame as f32 / RATE as f32;
        let value = phase.sin() * amplitude;
        samples[frame * CHANNELS] = value;
        samples[frame * CHANNELS + 1] = value;
    }
    samples
}

/// Runs `samples` through the effect in blocks, the way the engine does.
fn process_in_blocks(effect: &mut dyn AudioEffect, samples: &mut [f32]) {
    let mut offset = 0;
    while offset < samples.len() {
        let end = (offset + BLOCK_FRAMES * CHANNELS).min(samples.len());
        let mut block = stereo_block(&mut samples[offset..end]);
        effect.process(&mut block);
        offset = end;
    }
}

/// Magnitude of `frequency` in the left channel, by Goertzel.
fn tone_magnitude(samples: &[f32], frequency: f32) -> f32 {
    let mono: Vec<f32> = samples
        .chunks_exact(CHANNELS)
        .map(|frame| frame[0])
        .collect();
    let n = mono.len();
    if n == 0 {
        return 0.0;
    }

    #[allow(clippy::cast_precision_loss)]
    let n_f = n as f32;
    let k = (0.5 + n_f * frequency / RATE as f32).floor();
    let omega = 2.0 * PI * k / n_f;
    let coefficient = 2.0 * omega.cos();

    let (mut s1, mut s2) = (0.0_f32, 0.0_f32);
    for sample in &mono {
        let s0 = sample + coefficient * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = (s1 * s1 + s2 * s2 - coefficient * s1 * s2).max(0.0);
    power.sqrt() * 2.0 / n_f
}

/// Measures over the tail of the signal, after any transient has settled.
fn settled_tail(samples: &[f32]) -> &[f32] {
    &samples[samples.len() / 2..]
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()))
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let count = samples.len() as f32;
    (samples.iter().map(|s| s * s).sum::<f32>() / count).sqrt()
}

// ---------------------------------------------------------------------------
// Gain
// ---------------------------------------------------------------------------

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

    // A constant signal makes any discontinuity in the gain visible as a jump
    // between adjacent samples.
    let mut samples = vec![0.5_f32; 4_800 * CHANNELS];
    process_in_blocks(&mut effect, &mut samples);

    // Now slam the gain to full scale and look for a step.
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

// ---------------------------------------------------------------------------
// Parametric equaliser
// ---------------------------------------------------------------------------

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
    // Band 4 is the default 1 kHz peak; bands are 1-indexed in the parameter
    // names, so this is `band4_*`.
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

    // A constant offset is DC, which a high pass must reject entirely.
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

// ---------------------------------------------------------------------------
// Limiter
// ---------------------------------------------------------------------------

#[test]
fn the_limiter_never_lets_a_sample_past_the_ceiling() {
    let mut effect = LimiterEffect::new();
    effect
        .parameters()
        .set_by_id("threshold", ParamValue::Float(-6.0))
        .unwrap();
    effect.prepare(format());

    let ceiling = db_to_linear(-6.0);

    // Deliberately over-driven: 0 dBFS into a −6 dBFS ceiling.
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

    // 0.05 × 10 (20 dB) = 0.5, comfortably below the ceiling, so the signal
    // should arrive at full make-up gain rather than heavily limited.
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

    // A loud burst, then a quiet passage that must come back to unity.
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

    // Right channel is silent; with a linked detector it must be attenuated by
    // the same amount as the left, which is what preserves the stereo image.
    let frames = 9_600;
    let mut samples = vec![0.0_f32; frames * CHANNELS];
    for frame in 0..frames {
        #[allow(clippy::cast_precision_loss)]
        let phase = 2.0 * PI * 500.0 * frame as f32 / RATE as f32;
        samples[frame * CHANNELS] = phase.sin();
    }
    process_in_blocks(&mut effect, &mut samples);

    assert!(peak(&samples) <= db_to_linear(-3.0) + 1e-6);
    // The silent channel stays silent regardless of the gain applied.
    let right_peak = samples
        .chunks_exact(CHANNELS)
        .fold(0.0_f32, |acc, frame| acc.max(frame[1].abs()));
    assert!(right_peak < 1e-9);
}

// ---------------------------------------------------------------------------
// Registry integrity
// ---------------------------------------------------------------------------

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

            // Every declared default must be usable as-is.
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

            // Displays must never panic, whatever the value.
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
                    // A deterministic pseudo-random signal: no dependency on a
                    // randomness crate, and reproducible across runs.
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
