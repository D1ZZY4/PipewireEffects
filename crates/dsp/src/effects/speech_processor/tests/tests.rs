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
