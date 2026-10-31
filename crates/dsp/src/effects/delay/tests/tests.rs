use super::*;

fn impulse(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer.channel_mut(0)[0] = 1.0;
    buffer.channel_mut(1)[0] = 1.0;
    buffer
}

#[test]
fn a_delayed_impulse_reappears_after_the_delay_time() {
    let params = ParamSet::new(specs());
    params.set_f32("delay_left", 10.0).unwrap();
    params.set_f32("delay_right", 10.0).unwrap();
    params.set_f32("mix", 100.0).unwrap();
    params.set_f32("feedback", 0.0).unwrap();
    let mut effect = Delay::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = impulse(2048);
    effect.process(&mut buffer);
    let plan = buffer.channel(0);
    let peak_index = plan
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(index, _)| index)
        .unwrap();
    assert!(plan[peak_index].abs() > 0.5, "no echo found");
    assert!(peak_index.abs_diff(480) < 90, "echo at {peak_index}");
}

#[test]
fn feedback_decays_and_never_blows_up() {
    let params = ParamSet::new(specs());
    params.set_f32("feedback", 95.0).unwrap();
    params.set_f32("mix", 100.0).unwrap();
    let mut effect = Delay::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = impulse(1024);
    effect.process(&mut buffer);
    for _ in 0..2000 {
        let mut buffer = impulse(1024);
        buffer.channel_mut(0)[0] = 0.0;
        buffer.channel_mut(1)[0] = 0.0;
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 2.0, "{sample}");
            }
        }
    }
}

#[test]
fn a_synced_delay_follows_the_tempo() {
    let params = ParamSet::new(specs());
    params.set_f32("tempo", 120.0).unwrap();
    params.set_choice("sync_left", 3).unwrap();
    params.set_choice("sync_right", 3).unwrap();
    params.set_f32("mix", 100.0).unwrap();
    params.set_f32("feedback", 0.0).unwrap();
    let mut effect = Delay::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = impulse(30_000);
    effect.process(&mut buffer);
    let plan = buffer.channel(0);
    let peak_index = plan
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(index, _)| index)
        .unwrap();
    assert!(
        peak_index.abs_diff(24_000) < 90,
        "a quarter note at 120 BPM should land at 24 000 samples, the echo was at {peak_index}"
    );
}

#[test]
fn dry_signal_passes_through_with_zero_mix() {
    let params = ParamSet::new(specs());
    params.set_f32("mix", 0.0).unwrap();
    let mut effect = Delay::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = impulse(512);
    effect.process(&mut buffer);
    assert!((buffer.channel(0)[0] - 1.0).abs() < 1.0e-6);
}
