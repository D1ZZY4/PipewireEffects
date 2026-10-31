use super::*;

fn tone(frequency: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let sign = if channel == 1 { -1.0 } else { 1.0 };
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = sign
                * (std::f32::consts::TAU * frequency * (offset + index) as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn rms(effect: &mut VoiceSuppressor, inverted_channels: bool) -> f32 {
    let frames = 512;
    let mut total = 0.0f64;
    let mut count = 0usize;
    for block in 0..48 {
        let mut buffer = tone(1_000.0, frames, block * frames);
        if !inverted_channels {
            let right = buffer.channel(0).to_vec();
            buffer.channel_mut(1).copy_from_slice(&right);
        }
        effect.process(&mut buffer);
        if block >= 16 {
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    total += f64::from(*sample) * f64::from(*sample);
                    count += 1;
                }
            }
        }
    }
    (total / count as f64).sqrt() as f32
}

fn prepared(min_kurtosis: Option<f32>) -> VoiceSuppressor {
    let params = ParamSet::new(specs());
    if let Some(value) = min_kurtosis {
        params.set_f32("min_kurtosis", value).unwrap();
    }
    let mut effect = VoiceSuppressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

#[test]
fn centred_voice_is_attenuated() {
    let level = rms(&mut prepared(None), false);
    assert!(level < 0.15, "the centre was not suppressed: {level}");
}

#[test]
fn an_out_of_phase_signal_is_preserved() {
    let level = rms(&mut prepared(None), true);
    assert!(level > 0.5, "the side signal was suppressed: {level}");
}

#[test]
fn zero_mix_is_transparent() {
    let params = ParamSet::new(specs());
    params.set_f32("mix", 0.0).unwrap();
    let mut effect = VoiceSuppressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let level = rms(&mut effect, false);
    assert!(level > 0.5, "the signal was altered at zero mix: {level}");
}

#[test]
fn the_instantaneous_frequency_gate_keeps_a_fast_bin() {
    let params = ParamSet::new(specs());
    params.set_f32("max_if", 500.0).unwrap();
    let mut effect = VoiceSuppressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let level = rms(&mut effect, false);
    assert!(
        level > 0.5,
        "a 1 kHz centre was suppressed below the instantaneous frequency gate: {level}"
    );
}

#[test]
fn the_minimum_kurtosis_gate_keeps_a_bin_below_the_threshold() {
    let level = rms(&mut prepared(Some(100.0)), false);
    assert!(
        level > 0.5,
        "a centre was suppressed above the minimum kurtosis: {level}"
    );
}

#[test]
fn a_flat_spectrum_reads_below_a_peaked_one() {
    let flat = vec![1.0f32; BINS];
    let mut peak = vec![0.0f32; BINS];
    peak[BINS / 2] = 1.0;
    let flat = spectral_kurtosis(&flat, BINS / 2);
    let peak = spectral_kurtosis(&peak, BINS / 2);
    assert!(flat < 1.0, "a flat triplet read {flat}");
    assert!(peak > flat, "a peak read {peak} against a flat {flat}");
}
