use std::f32::consts::TAU;

use crate::effects::vocoder::{CHANNELS, FFT_SIZE, HOP};
use crate::fft::Fft;

use super::*;

const RATE: u32 = 48_000;

fn tone(frequency: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(CHANNELS, frames);
    buffer.configure(CHANNELS, frames).unwrap();
    for channel in 0..CHANNELS {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = TAU * frequency * (offset + index) as f32 / RATE as f32;
            *sample = 0.5 * phase.sin();
        }
    }
    buffer
}

fn run(parameters: &[(&str, f32)], frequency: f32, blocks: usize) -> Vec<f32> {
    let params = ParamSet::new(specs());
    for (id, value) in parameters {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = PitchShift::new(params);
    effect.prepare(AudioFormat::new(RATE, CHANNELS));
    let mut collected = Vec::new();
    let frames = FFT_SIZE;
    for block in 0..blocks {
        let mut buffer = tone(frequency, frames, block * frames);
        effect.process(&mut buffer);
        collected.extend_from_slice(buffer.channel(0));
    }
    collected
}

fn dominant_frequency(samples: &[f32]) -> f32 {
    let size = 4096;
    let fft = Fft::new(size);
    let mut re = vec![0.0f32; size];
    let mut im = vec![0.0f32; size];
    for (index, value) in samples.iter().take(size).enumerate() {
        let angle = TAU * index as f32 / size as f32;
        re[index] = value * (0.5 - 0.5 * angle.cos());
    }
    fft.forward(&mut re, &mut im);
    let mut best = 1usize;
    for bin in 1..size / 2 {
        let power = re[bin] * re[bin] + im[bin] * im[bin];
        let winning = re[best] * re[best] + im[best] * im[best];
        if power > winning {
            best = bin;
        }
    }
    best as f32 * RATE as f32 / size as f32
}

fn steady(samples: &[f32]) -> &[f32] {
    &samples[samples.len() - 4096..]
}

#[test]
fn an_octave_up_doubles_the_frequency() {
    let samples = run(&[("semitones", 12.0)], 1_000.0, 8);
    let measured = dominant_frequency(steady(&samples));
    assert!((measured - 2_000.0).abs() < 60.0, "{measured}");
}

#[test]
fn an_octave_down_halves_the_frequency() {
    let samples = run(&[("semitones", -12.0)], 2_000.0, 8);
    let measured = dominant_frequency(steady(&samples));
    assert!((measured - 1_000.0).abs() < 60.0, "{measured}");
}

#[test]
fn no_shift_keeps_the_frequency() {
    let samples = run(&[("semitones", 0.0)], 1_000.0, 8);
    let measured = dominant_frequency(steady(&samples));
    assert!((measured - 1_000.0).abs() < 60.0, "{measured}");
}

#[test]
fn cents_fine_tune_the_ratio() {
    let samples = run(&[("semitones", 0.0), ("cents", 100.0)], 1_000.0, 8);
    let measured = dominant_frequency(steady(&samples));
    assert!((measured - 1_059.5).abs() < 40.0, "{measured}");
}

#[test]
fn the_declared_latency_places_the_impulse() {
    let params = ParamSet::new(specs());
    let mut effect = PitchShift::new(params);
    effect.prepare(AudioFormat::new(RATE, CHANNELS));
    let mut samples = Vec::new();
    for block in 0..4 {
        let mut buffer = AudioBuffer::new(CHANNELS, FFT_SIZE);
        buffer.configure(CHANNELS, FFT_SIZE).unwrap();
        if block == 0 {
            buffer.channel_mut(0)[0] = 1.0;
            buffer.channel_mut(1)[0] = 1.0;
        }
        effect.process(&mut buffer);
        samples.extend_from_slice(buffer.channel(0));
    }
    let peak = samples
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(index, _)| index)
        .unwrap();
    assert!(
        peak.abs_diff(LATENCY_FRAMES) <= HOP,
        "the impulse landed at {peak}, not near {LATENCY_FRAMES}"
    );
}

#[test]
fn a_zero_mix_is_the_input_untouched() {
    let samples = run(&[("semitones", 12.0), ("mix", 0.0)], 1_000.0, 4);
    for (index, sample) in samples.iter().enumerate() {
        let expected = 0.5 * (TAU * 1_000.0 * index as f32 / RATE as f32).sin();
        assert!((sample - expected).abs() < 1.0e-6, "{index}");
    }
}

#[test]
fn the_output_stays_finite_under_the_widest_settings() {
    let params = ParamSet::new(specs());
    params.set_f32("semitones", 24.0).unwrap();
    params.set_f32("cents", 100.0).unwrap();
    let mut effect = PitchShift::new(params);
    effect.prepare(AudioFormat::new(RATE, CHANNELS));
    for block in 0..6 {
        let mut buffer = tone(400.0, FFT_SIZE, block * FFT_SIZE);
        effect.process(&mut buffer);
        for channel in 0..CHANNELS {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 4.0, "{sample}");
            }
        }
    }
    assert_eq!(effect.latency_frames(), LATENCY_FRAMES);
}

fn burst(parameters: &[(&str, f32)]) -> Vec<f32> {
    let params = ParamSet::new(specs());
    for (id, value) in parameters {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = PitchShift::new(params);
    effect.prepare(AudioFormat::new(RATE, CHANNELS));
    let mut collected = Vec::new();
    for block in 0..8 {
        let mut buffer = AudioBuffer::new(CHANNELS, FFT_SIZE);
        buffer.configure(CHANNELS, FFT_SIZE).unwrap();
        if block >= 2 {
            let offset = (block - 2) * FFT_SIZE;
            for channel in 0..CHANNELS {
                let plan = buffer.channel_mut(channel);
                for (index, sample) in plan.iter_mut().enumerate() {
                    *sample = 0.5 * (TAU * 1_000.0 * (offset + index) as f32 / RATE as f32).sin();
                }
            }
            if block == 2 {
                buffer.channel_mut(0)[0] += 1.0;
                buffer.channel_mut(1)[0] += 1.0;
            }
        }
        effect.process(&mut buffer);
        collected.extend_from_slice(buffer.channel(0));
    }
    collected
}

fn difference(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max)
}

#[test]
fn the_quality_controls_default_to_the_previous_behaviour() {
    let params = ParamSet::new(specs());
    let effect = PitchShift::new(params);
    assert_eq!(effect.transients.index(), 0);
    assert_eq!(effect.detector.index(), 0);
    assert_eq!(effect.phase.index(), 0);
    assert_eq!(effect.options(), VocoderOptions::default());
}

#[test]
fn laminar_phase_locks_the_render_but_keeps_the_shift() {
    let laminar = run(&[("semitones", 12.0), ("phase", 1.0)], 1_000.0, 8);
    let measured = dominant_frequency(steady(&laminar));
    assert!((measured - 2_000.0).abs() < 60.0, "{measured}");
    let independent = run(&[("semitones", 12.0), ("phase", 0.0)], 1_000.0, 8);
    assert!(
        difference(steady(&laminar), steady(&independent)) > 1.0e-3,
        "the phase mode did not change the render"
    );
}

#[test]
fn crisp_transients_reset_the_phases_on_an_onset() {
    let smooth = burst(&[("transients", 0.0)]);
    let mixed = burst(&[("transients", 1.0)]);
    let crisp = burst(&[("transients", 2.0)]);
    assert!(difference(&smooth, &mixed) > 1.0e-3, "mixed matched smooth");
    assert!(difference(&smooth, &crisp) > 1.0e-3, "crisp matched smooth");
}

#[test]
fn the_transient_default_matches_the_smooth_setting() {
    let default = burst(&[]);
    let smooth = burst(&[("transients", 0.0)]);
    assert!(difference(&default, &smooth) < 1.0e-9);
}

#[test]
fn the_detector_modes_set_distinct_sensitivities() {
    assert_eq!(
        TransientDetector::from_index(0),
        TransientDetector::Compound
    );
    assert_eq!(
        TransientDetector::from_index(1),
        TransientDetector::Percussive
    );
    assert_eq!(TransientDetector::from_index(2), TransientDetector::Soft);
    assert!(
        TransientDetector::Percussive.sensitivity() < TransientDetector::Compound.sensitivity()
    );
    assert!(TransientDetector::Compound.sensitivity() < TransientDetector::Soft.sensitivity());
}
