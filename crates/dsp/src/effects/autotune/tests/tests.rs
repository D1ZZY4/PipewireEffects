use std::f32::consts::TAU;

use crate::fft::{Fft, hann};

use super::*;

const RATE: u32 = 48_000;
const BLOCK: usize = 1_024;

fn tone(frequency: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = TAU * frequency * (offset + index) as f32 / RATE as f32;
            *sample = 0.5 * phase.sin();
        }
    }
    buffer
}

fn dominant(samples: &[f32]) -> f32 {
    let size = 8_192;
    let fft = Fft::new(size);
    let window = hann(size);
    let mut re = vec![0.0f32; size];
    let mut im = vec![0.0f32; size];
    for (index, slot) in re.iter_mut().enumerate() {
        *slot = samples.get(index).copied().unwrap_or(0.0) * window[index];
    }
    fft.forward(&mut re, &mut im);
    let mut best = 1usize;
    for bin in 1..size / 2 {
        let power = re[bin] * re[bin] + im[bin] * im[bin];
        if power > re[best] * re[best] + im[best] * im[best] {
            best = bin;
        }
    }
    best as f32 * RATE as f32 / size as f32
}

fn run_effect(frequency: f32, settings: &[(&str, f32)], blocks: usize) -> (Vec<f32>, Autotune) {
    let params = ParamSet::new(specs());
    for (id, value) in settings {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = Autotune::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    let mut collected = Vec::new();
    for block in 0..blocks {
        let mut buffer = tone(frequency, BLOCK, block * BLOCK);
        effect.process(&mut buffer);
        collected.extend_from_slice(buffer.channel(0));
    }
    (collected, effect)
}

fn run(frequency: f32, settings: &[(&str, f32)], blocks: usize) -> Vec<f32> {
    run_effect(frequency, settings, blocks).0
}

#[test]
fn a_detuned_pitch_is_pulled_to_the_nearest_note() {
    let samples = run(226.0, &[], 24);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!(
        (measured - 220.0).abs() < 8.0,
        "the pitch was not corrected to A3: {measured}"
    );
}

#[test]
fn zero_strength_leaves_the_pitch_alone() {
    let samples = run(226.0, &[("strength", 0.0)], 24);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!(
        (measured - 226.0).abs() < 8.0,
        "the pitch moved with zero strength: {measured}"
    );
}

#[test]
fn zero_mix_is_transparent() {
    let mut effect = {
        let params = ParamSet::new(specs());
        params.set_f32("mix", 0.0).unwrap();
        let mut effect = Autotune::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        effect
    };
    let mut buffer = tone(226.0, BLOCK, 0);
    let reference = buffer.channel(0).to_vec();
    effect.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
        assert!((processed - original).abs() < 1.0e-6);
    }
}

#[test]
fn the_output_stays_finite_under_the_widest_settings() {
    let mut effect = {
        let params = ParamSet::new(specs());
        params.set_f32("strength", 100.0).unwrap();
        params.set_f32("speed", 5.0).unwrap();
        params.set_f32("min_frequency", 50.0).unwrap();
        params.set_f32("max_frequency", 2_000.0).unwrap();
        let mut effect = Autotune::new(params);
        effect.prepare(AudioFormat::new(RATE, 2));
        effect
    };
    for block in 0..16 {
        let mut buffer = tone(58.0, BLOCK, block * BLOCK);
        buffer.channel_mut(0)[0] = 4.0;
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 32.0, "{sample}");
            }
        }
    }
}

#[test]
fn the_chromatic_scale_is_the_default_target_set() {
    let params = ParamSet::new(specs());
    let effect = Autotune::new(params);
    assert_eq!(effect.scale.index(), 0);
    assert!((effect.quantise(57.463) - 57.0).abs() < 1.0e-6);
    assert!((effect.quantise(57.6) - 58.0).abs() < 1.0e-6);
    assert!((effect.quantise(60.0) - 60.0).abs() < 1.0e-6);
}

#[test]
fn a_major_scale_targets_only_its_tones() {
    let params = ParamSet::new(specs());
    params.set_f32("scale", 1.0).unwrap();
    params.set_f32("root", 0.0).unwrap();
    let effect = Autotune::new(params);
    assert!((effect.quantise(61.0) - 60.0).abs() < 1.0e-6);
    assert!((effect.quantise(64.0) - 64.0).abs() < 1.0e-6);
    assert!((effect.quantise(66.4) - 67.0).abs() < 1.0e-6);
}

#[test]
fn a_minor_pentatonic_scale_targets_its_tones() {
    let params = ParamSet::new(specs());
    params.set_f32("scale", 5.0).unwrap();
    params.set_f32("root", 9.0).unwrap();
    let effect = Autotune::new(params);
    assert!((effect.quantise(60.0) - 60.0).abs() < 1.0e-6);
    assert!((effect.quantise(62.0) - 62.0).abs() < 1.0e-6);
    assert!((effect.quantise(65.0) - 64.0).abs() < 1.0e-6);
}

#[test]
fn the_root_note_selects_the_scale() {
    let params = ParamSet::new(specs());
    params.set_f32("scale", 1.0).unwrap();
    params.set_f32("root", 2.0).unwrap();
    let effect = Autotune::new(params);
    assert!((effect.quantise(61.0) - 61.0).abs() < 1.0e-6);
    assert!((effect.quantise(60.0) - 59.0).abs() < 1.0e-6);
}

#[test]
fn the_scale_mask_changes_the_rendered_pitch() {
    let chromatic = run(270.0, &[], 24);
    let major = run(270.0, &[("scale", 1.0), ("root", 0.0)], 24);
    let chromatic_pitch = dominant(&chromatic[chromatic.len() - 8_192..]);
    let major_pitch = dominant(&major[major.len() - 8_192..]);
    assert!((chromatic_pitch - 277.18).abs() < 8.0, "{chromatic_pitch}");
    assert!((major_pitch - 261.63).abs() < 8.0, "{major_pitch}");
}

#[test]
fn bend_range_limits_the_correction() {
    let mut wide = {
        let params = ParamSet::new(specs());
        params.set_f32("bend_range", 7.0).unwrap();
        Autotune::new(params)
    };
    let mut narrow = {
        let params = ParamSet::new(specs());
        params.set_f32("bend_range", 0.1).unwrap();
        Autotune::new(params)
    };
    let mut none = {
        let params = ParamSet::new(specs());
        params.set_f32("bend_range", 0.0).unwrap();
        Autotune::new(params)
    };
    let full = wide.correction_ratio(226.0, 0.0, 7.0);
    let limited = narrow.correction_ratio(226.0, 0.0, 0.1);
    let off = none.correction_ratio(226.0, 0.0, 0.0);
    assert!((full - 220.0 / 226.0).abs() < 1.0e-3, "{full}");
    assert!((off - 1.0).abs() < 1.0e-6, "{off}");
    assert!(
        (limited - 1.0).abs() < (full - 1.0).abs(),
        "{limited} was not limited against {full}"
    );
    assert!(limited < 1.0, "{limited}");
}

#[test]
fn the_target_offset_shifts_the_correction() {
    let params = ParamSet::new(specs());
    params.set_f32("offset", 1.0).unwrap();
    params.set_f32("bend_range", 7.0).unwrap();
    let mut effect = Autotune::new(params);
    let ratio = effect.correction_ratio(226.0, 1.0, 7.0);
    let expected = 233.082 / 226.0;
    assert!((ratio - expected).abs() < 5.0e-3, "{ratio}");
}

#[test]
fn the_detection_filter_smooths_pitch_changes() {
    let mut responsive = {
        let params = ParamSet::new(specs());
        params.set_f32("filter", 0.02).unwrap();
        Autotune::new(params)
    };
    let mut smooth = {
        let params = ParamSet::new(specs());
        params.set_f32("filter", 0.5).unwrap();
        Autotune::new(params)
    };
    let _ = responsive.correction_ratio(220.0, 0.0, 7.0);
    let _ = smooth.correction_ratio(220.0, 0.0, 7.0);
    let _ = responsive.correction_ratio(233.0, 0.0, 7.0);
    let _ = smooth.correction_ratio(233.0, 0.0, 7.0);
    assert!(
        responsive.smoothed > smooth.smoothed + 0.2,
        "{} did not track faster than {}",
        responsive.smoothed,
        smooth.smoothed
    );
}

#[test]
fn fast_mode_applies_the_correction_within_the_first_blocks() {
    let (_, normal) = run_effect(226.0, &[], 4);
    let (_, fast) = run_effect(226.0, &[("fast_mode", 1.0)], 4);
    let target = 220.0 / 226.0;
    assert!(
        (fast.ratio - target).abs() < 1.0e-3,
        "fast ratio {} did not reach {target}",
        fast.ratio
    );
    assert!(
        (fast.ratio - target).abs() < (normal.ratio - target).abs(),
        "fast {} was not ahead of normal {}",
        fast.ratio,
        normal.ratio
    );
    let samples = run(226.0, &[("fast_mode", 1.0)], 8);
    let measured = dominant(&samples[samples.len() - 8_192..]);
    assert!((measured - 220.0).abs() < 8.0, "{measured}");
}

#[test]
fn the_fast_mode_flag_defaults_to_off() {
    let params = ParamSet::new(specs());
    let effect = Autotune::new(params);
    assert!(!effect.fast_mode.flag());
    assert!((effect.bend_range.value() - 2.0).abs() < 1.0e-6);
    assert!((effect.filter.value() - 0.02).abs() < 1.0e-6);
}
