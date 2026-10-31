use super::*;

fn tone(amplitude: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = std::f32::consts::TAU * 220.0 * (offset + index) as f32 / 48_000.0;
            *sample = amplitude * phase.sin();
        }
    }
    buffer
}

fn prepared(settings: &[(&str, f32)]) -> SpeechProcessor {
    let params = ParamSet::new(specs());
    for (id, value) in settings {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = SpeechProcessor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

fn decaying(amplitude: f32, frames: usize, offset: usize, tau: f32) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let position = (offset + index) as f32 / 48_000.0;
            let phase = std::f32::consts::TAU * 500.0 * position;
            *sample = amplitude * (-position / tau).exp() * phase.sin();
        }
    }
    buffer
}

fn energy(buffer: &AudioBuffer) -> f64 {
    let mut sum = 0.0;
    for channel in 0..2 {
        for sample in buffer.channel(channel) {
            sum += f64::from(*sample) * f64::from(*sample);
        }
    }
    sum
}

fn settle(effect: &mut SpeechProcessor, amplitude: f32) -> f32 {
    let frames = 4_800;
    let mut buffer = tone(amplitude, frames, 0);
    for block in 0..120 {
        buffer = tone(amplitude, frames, block * frames);
        effect.process(&mut buffer);
    }
    buffer.peak()
}

#[test]
fn a_steady_hiss_is_reduced_when_nothing_is_speaking() {
    let mut effect = prepared(&[("agc", 0.0), ("mix", 100.0)]);
    let level = settle(&mut effect, 10f32.powf(-40.0 / 20.0));
    assert!(
        level < 10f32.powf(-40.0 / 20.0) * 0.5,
        "the noise floor was not reduced: {level}"
    );
}

#[test]
fn the_denoiser_can_be_switched_off() {
    let mut effect = prepared(&[("agc", 0.0), ("denoise", 0.0)]);
    let level = settle(&mut effect, 10f32.powf(-40.0 / 20.0));
    assert!(
        (level - 10f32.powf(-40.0 / 20.0)).abs() < 1.0e-3,
        "the signal changed with the denoiser off: {level}"
    );
}

#[test]
fn a_voice_that_stands_out_is_lifted_towards_the_target() {
    let mut effect = prepared(&[("denoise", 0.0)]);
    settle(&mut effect, 0.01);
    let frames = 4_800;
    for block in 0..600 {
        let mut buffer = tone(0.01, frames, block * frames);
        effect.process(&mut buffer);
    }
    assert!(effect.gain_db > 3.0, "{}", effect.gain_db);
}

#[test]
fn zero_mix_is_transparent() {
    let mut effect = prepared(&[("mix", 0.0)]);
    let mut buffer = tone(0.5, 4_800, 0);
    let reference = buffer.channel(0).to_vec();
    effect.process(&mut buffer);
    for (processed, original) in buffer.channel(0).iter().zip(reference.iter()) {
        assert!((processed - original).abs() < 1.0e-6);
    }
}

#[test]
fn the_output_stays_finite_at_the_extremes() {
    let mut effect = prepared(&[
        ("vad_threshold", 3.0),
        ("max_gain", 30.0),
        ("depth", 60.0),
        ("target", -5.0),
    ]);
    for block in 0..200 {
        let mut buffer = tone(0.0001, 4_800, block * 4_800);
        buffer.channel_mut(0)[0] = 4.0;
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 64.0, "{sample}");
            }
        }
    }
}

#[test]
fn the_dereverb_reduces_a_decaying_tail() {
    let settings = [("denoise", 0.0), ("agc", 0.0), ("mix", 100.0)];
    let mut plain = prepared(&settings);
    let mut dereverbed = prepared(&[&settings[..], &[("dereverb", 1.0f32)]].concat());
    let frames = 512;
    let mut plain_energy = 0.0f64;
    let mut dereverbed_energy = 0.0f64;
    for block in 0..300 {
        let offset = block * frames;
        let mut dry = decaying(0.5, frames, offset, 0.06);
        let mut wet = decaying(0.5, frames, offset, 0.06);
        plain.process(&mut dry);
        dereverbed.process(&mut wet);
        if block >= 30 {
            plain_energy += energy(&dry);
            dereverbed_energy += energy(&wet);
        }
    }
    assert!(
        dereverbed_energy < plain_energy * 0.5,
        "the dereverbed tail carried {dereverbed_energy} against {plain_energy}"
    );
}

#[test]
fn the_dereverb_leaves_a_steady_tone_alone() {
    let settings = [("denoise", 0.0), ("agc", 0.0), ("mix", 100.0)];
    let mut plain = prepared(&settings);
    let mut dereverbed = prepared(&[&settings[..], &[("dereverb", 1.0f32)]].concat());
    let frames = 512;
    let mut plain_energy = 0.0f64;
    let mut dereverbed_energy = 0.0f64;
    for block in 0..120 {
        let mut dry = tone(0.3, frames, block * frames);
        let mut wet = tone(0.3, frames, block * frames);
        plain.process(&mut dry);
        dereverbed.process(&mut wet);
        if block >= 90 {
            plain_energy += energy(&dry);
            dereverbed_energy += energy(&wet);
        }
    }
    let difference = 10.0 * (dereverbed_energy / plain_energy).log10();
    assert!(difference.abs() < 1.0, "{difference} dB");
}

#[test]
fn the_dereverb_reports_its_latency_only_when_enabled() {
    let plain = prepared(&[]);
    let dereverbed = prepared(&[("dereverb", 1.0f32)]);
    assert_eq!(plain.latency_frames(), 0);
    assert_eq!(dereverbed.latency_frames(), 384);
}
