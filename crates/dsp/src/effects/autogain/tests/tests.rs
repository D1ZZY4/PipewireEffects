use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 997.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn feed(effect: &mut AutoGain, amplitude: f32, blocks: usize) {
    for _ in 0..blocks {
        let mut buffer = tone(amplitude, 4800);
        effect.process(&mut buffer);
    }
}

fn measured(amplitude: f32, blocks: usize) -> f32 {
    let mut loudness = Loudness::new(48_000);
    for _ in 0..blocks {
        let buffer = tone(amplitude, 4800);
        loudness.process(&buffer);
    }
    loudness.integrated().expect("a level was measured")
}

#[test]
fn a_full_scale_sine_measures_about_zero_lufs() {
    let value = measured(1.0, 40);
    assert!(value.abs() < 0.6, "{value}");
}

#[test]
fn the_standard_calibration_holds() {
    let amplitude = 10f32.powf(-23.0 / 20.0);
    let value = measured(amplitude, 40);
    assert!((value + 23.0).abs() < 0.4, "{value}");
}

#[test]
fn a_ten_decibel_step_moves_the_measurement_by_ten() {
    let loud = measured(0.5, 40);
    let quiet = measured(0.5 / 10f32.powf(10.0 / 20.0), 40);
    assert!((loud - quiet - 10.0).abs() < 0.3, "{loud} {quiet}");
}

#[test]
fn silence_is_not_measured() {
    let mut loudness = Loudness::new(48_000);
    let mut silent = AudioBuffer::new(2, 4800);
    silent.configure(2, 4800).unwrap();
    for _ in 0..40 {
        loudness.process(&silent);
    }
    assert!(loudness.integrated().is_none());
}

#[test]
fn quiet_material_is_lifted_towards_the_target() {
    let params = ParamSet::new(specs());
    params.set_f32("speed", 100.0).unwrap();
    let mut effect = AutoGain::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    feed(&mut effect, 0.01, 400);
    assert!(effect.gain_db > 5.0, "{}", effect.gain_db);
    assert!(effect.gain_db <= 20.0);
}

#[test]
fn the_gain_is_capped_by_the_maximum() {
    let params = ParamSet::new(specs());
    params.set_f32("speed", 100.0).unwrap();
    params.set_f32("max_gain", 6.0).unwrap();
    let mut effect = AutoGain::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    feed(&mut effect, 0.0001, 400);
    assert!(effect.gain_db <= 6.001, "{}", effect.gain_db);
}

#[test]
fn silence_does_not_change_the_gain() {
    let params = ParamSet::new(specs());
    let mut effect = AutoGain::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut silent = AudioBuffer::new(2, 4800);
    silent.configure(2, 4800).unwrap();
    for _ in 0..100 {
        effect.process(&mut silent);
    }
    assert_eq!(effect.gain_db, 0.0);
}

#[test]
fn no_gain_is_applied_when_the_loudness_already_matches() {
    let params = ParamSet::new(specs());
    params.set_f32("target", -23.0).unwrap();
    let mut effect = AutoGain::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    feed(&mut effect, 10f32.powf(-23.0 / 20.0), 60);
    assert!(effect.gain_db.abs() < 0.5, "{}", effect.gain_db);
}
