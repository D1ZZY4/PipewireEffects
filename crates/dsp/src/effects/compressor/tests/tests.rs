use super::*;

fn low_tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 60.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 400.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

#[test]
fn signal_below_the_threshold_is_untouched() {
    let params = ParamSet::new(specs());
    let mut effect = Compressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(0.05, 4096);
    for _ in 0..16 {
        effect.process(&mut buffer);
    }
    assert!(buffer.peak() <= 0.0501, "{}", buffer.peak());
}

#[test]
fn signal_above_the_threshold_is_reduced() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -24.0).unwrap();
    params.set_f32("ratio", 8.0).unwrap();
    params.set_f32("knee", 0.0).unwrap();
    params.set_f32("attack", 1.0).unwrap();
    let mut effect = Compressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(0.9, 1024);
    for _ in 0..256 {
        buffer = tone(0.9, 1024);
        effect.process(&mut buffer);
    }
    let output_db = math::gain_to_db(buffer.peak());
    assert!(output_db < -18.0, "not enough reduction: {output_db}");
    assert!(output_db > -24.0, "too much reduction: {output_db}");
}

#[test]
fn a_higher_ratio_compresses_harder() {
    let measure = |ratio: f32| {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -30.0).unwrap();
        params.set_f32("ratio", ratio).unwrap();
        params.set_f32("knee", 0.0).unwrap();
        params.set_f32("attack", 1.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.9, 1024);
        for _ in 0..256 {
            buffer = tone(0.9, 1024);
            effect.process(&mut buffer);
        }
        buffer.peak()
    };
    let gentle = measure(2.0);
    let aggressive = measure(16.0);
    assert!(aggressive < gentle, "{aggressive} !< {gentle}");
}

#[test]
fn makeup_gain_is_added_back() {
    let params = ParamSet::new(specs());
    params.set_f32("makeup", 6.0).unwrap();
    let mut effect = Compressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(0.1, 4096);
    effect.process(&mut buffer);
    assert!((buffer.peak() - 0.1 * math::db_to_gain(6.0)).abs() < 0.01);
}

#[test]
fn the_sidechain_high_pass_keeps_rumble_out_of_the_detector() {
    let measure = |mode: u16| {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -24.0).unwrap();
        params.set_f32("ratio", 8.0).unwrap();
        params.set_f32("knee", 0.0).unwrap();
        params.set_f32("attack", 1.0).unwrap();
        params.set_choice("hpf_mode", mode).unwrap();
        params.set_f32("hpf_frequency", 400.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = low_tone(0.9, 4096);
        for _ in 0..64 {
            effect.process(&mut buffer);
        }
        buffer.peak()
    };
    let unfiltered = measure(0);
    let filtered = measure(1);
    assert!(
        unfiltered < 0.4,
        "the rumble was not compressed at all: {unfiltered}"
    );
    assert!(
        filtered > unfiltered * 2.0,
        "the detector filter did not change the reduction: {filtered} against {unfiltered}"
    );
}

#[test]
fn half_mix_blends_dry_and_wet() {
    let params = ParamSet::new(specs());
    params.set_f32("threshold", -24.0).unwrap();
    params.set_f32("mix", 0.0).unwrap();
    let mut effect = Compressor::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    let mut buffer = tone(0.9, 4096);
    effect.process(&mut buffer);
    assert!((buffer.peak() - 0.9).abs() < 0.01, "{}", buffer.peak());
}
