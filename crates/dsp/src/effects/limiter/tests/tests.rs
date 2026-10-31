use super::*;

fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = amplitude * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

fn prepared(params: ParamSet) -> Limiter {
    let mut effect = Limiter::new(params);
    effect.prepare(AudioFormat::new(48_000, 2));
    effect
}

fn silence(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    buffer
}

#[test]
fn the_ceiling_is_never_exceeded() {
    let params = ParamSet::new(specs());
    params.set_f32("input_gain", 20.0).unwrap();
    params.set_f32("limit", -6.0).unwrap();
    let mut effect = prepared(params);
    let ceiling = math::db_to_gain(-6.0);
    for _ in 0..128 {
        let mut buffer = tone(1.0, 1024);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.abs() <= ceiling + 1.0e-6, "{sample} > {ceiling}");
            }
        }
    }
}

#[test]
fn a_limited_burst_keeps_its_shape_instead_of_being_clipped() {
    let params = ParamSet::new(specs());
    params.set_f32("limit", -6.0).unwrap();
    params.set_f32("lookahead", 20.0).unwrap();
    let mut effect = prepared(params);
    let ceiling = math::db_to_gain(-6.0);
    for _ in 0..4 {
        let mut buffer = tone(1.0, 8192);
        effect.process(&mut buffer);
        let steady = &buffer.channel(0)[4096..];
        let peak = steady.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let rms = (steady.iter().map(|v| v * v).sum::<f32>() / steady.len() as f32).sqrt();
        let crest = peak / rms;
        assert!(
            (crest - std::f32::consts::SQRT_2).abs() < 0.03,
            "the burst was clipped: its crest factor is {crest} instead of {}",
            std::f32::consts::SQRT_2
        );
        assert!((peak - ceiling).abs() < 0.01, "{peak} is not at {ceiling}");
    }
}

#[test]
fn quiet_signal_passes_through_unity() {
    let params = ParamSet::new(specs());
    let mut effect = prepared(params);
    for _ in 0..4 {
        let mut buffer = tone(0.05, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.05).abs() < 0.005, "{}", buffer.peak());
    }
}

#[test]
fn the_output_lags_by_the_lookahead() {
    let params = ParamSet::new(specs());
    params.set_f32("lookahead", 10.0).unwrap();
    let effect = prepared(params);
    assert_eq!(effect.latency_frames(), 480);
    let mut impulse = AudioBuffer::new(2, 1024);
    impulse.configure(2, 1024).unwrap();
    impulse.channel_mut(0)[0] = 0.25;
    impulse.channel_mut(1)[0] = 0.25;
    let mut effect = effect;
    effect.process(&mut impulse);
    assert!(impulse.channel(0)[..480].iter().all(|value| *value == 0.0));
    assert!((impulse.channel(0)[480] - 0.25).abs() < 1.0e-6);
}

#[test]
fn the_gain_recovers_after_a_transient() {
    let params = ParamSet::new(specs());
    params.set_f32("limit", -12.0).unwrap();
    params.set_f32("release", 5.0).unwrap();
    let mut effect = prepared(params);
    let mut burst = tone(1.0, 256);
    effect.process(&mut burst);
    let mut flush = AudioBuffer::new(2, 4096);
    flush.configure(2, 4096).unwrap();
    effect.process(&mut flush);
    let mut quiet = tone(0.02, 4096);
    for _ in 0..64 {
        effect.process(&mut quiet);
    }
    assert!((quiet.peak() - 0.02).abs() < 0.002, "{}", quiet.peak());
}

#[test]
fn dithering_off_keeps_digital_silence() {
    let mut effect = prepared(ParamSet::new(specs()));
    for _ in 0..4 {
        let mut buffer = silence(4096);
        effect.process(&mut buffer);
        assert!(buffer.peak() == 0.0, "{}", buffer.peak());
    }
}

#[test]
fn rectangular_dither_is_bounded_and_not_silence() {
    let params = ParamSet::new(specs());
    params.set_choice("dithering", 1).unwrap();
    let mut effect = prepared(params);
    let mut maximum = 0.0f32;
    let mut nonzero = 0usize;
    for _ in 0..8 {
        let mut buffer = silence(4096);
        effect.process(&mut buffer);
        for sample in buffer.channel(0) {
            maximum = maximum.max(sample.abs());
            if *sample != 0.0 {
                nonzero += 1;
            }
        }
    }
    assert!(nonzero > 0, "the rectangular dither added nothing");
    assert!(maximum <= 1.0 / 32_768.0 + 1.0e-10, "{maximum}");
}

fn dither_variance(mode: u16) -> (f64, f32) {
    let params = ParamSet::new(specs());
    params.set_choice("dithering", mode).unwrap();
    let mut effect = prepared(params);
    let mut sum = 0.0f64;
    let mut count = 0usize;
    let mut maximum = 0.0f32;
    for _ in 0..16 {
        let mut buffer = silence(4096);
        effect.process(&mut buffer);
        for sample in buffer.channel(0) {
            sum += f64::from(*sample) * f64::from(*sample);
            count += 1;
            maximum = maximum.max(sample.abs());
        }
    }
    (sum / count as f64, maximum)
}

#[test]
fn triangular_dither_is_bounded_with_a_lower_variance() {
    let (rectangular, rect_max) = dither_variance(1);
    let (triangular, tri_max) = dither_variance(2);
    assert!(tri_max <= 1.0 / 32_768.0 + 1.0e-10, "{tri_max}");
    assert!(rect_max <= 1.0 / 32_768.0 + 1.0e-10, "{rect_max}");
    assert!(
        triangular < rectangular * 0.7 && triangular > rectangular * 0.2,
        "the triangular distribution has {triangular} against {rectangular}"
    );
}

#[test]
fn a_longer_attack_holds_the_previous_level() {
    let output = |attack: f32| {
        let params = ParamSet::new(specs());
        params.set_f32("limit", -12.0).unwrap();
        params.set_f32("lookahead", 20.0).unwrap();
        params.set_f32("attack", attack).unwrap();
        let mut effect = prepared(params);
        let mut pre = tone(0.1, 4096);
        effect.process(&mut pre);
        let mut loud = tone(1.0, 4096);
        effect.process(&mut loud);
        loud.channel(0)[104].abs()
    };
    let long = output(20.0);
    let short = output(1.0);
    assert!(
        long > short * 2.0,
        "the attack did not change the ramp: {long} against {short}"
    );
}

#[test]
fn gain_boost_lifts_a_quiet_passage_after_a_loud_one() {
    let measure = |boost: bool| {
        let params = ParamSet::new(specs());
        params.set_f32("limit", -6.0).unwrap();
        params.set_f32("lookahead", 5.0).unwrap();
        params.set_f32("release", 5.0).unwrap();
        params.set_bool("gain_boost", boost).unwrap();
        let mut effect = prepared(params);
        for _ in 0..8 {
            let mut loud = tone(1.0, 4096);
            effect.process(&mut loud);
        }
        let mut peak = 0.0f32;
        for _ in 0..4 {
            let mut quiet = tone(0.02, 4096);
            effect.process(&mut quiet);
            peak = quiet.peak();
        }
        let mut loud = tone(1.0, 4096);
        effect.process(&mut loud);
        (peak, loud.peak())
    };
    let (boosted, ceiling) = measure(true);
    let (plain, _) = measure(false);
    assert!(
        boosted > plain * 1.15,
        "gain boost did not lift the quiet passage: {boosted} against {plain}"
    );
    assert!(
        ceiling <= math::db_to_gain(-6.0) + 1.0e-3,
        "gain boost broke the ceiling: {ceiling}"
    );
}

#[test]
fn alr_releases_faster_than_the_standard_mode() {
    let measure = |mode: u16| {
        let params = ParamSet::new(specs());
        params.set_f32("limit", -12.0).unwrap();
        params.set_f32("lookahead", 1.0).unwrap();
        params.set_f32("release", 50.0).unwrap();
        params.set_choice("release_mode", mode).unwrap();
        let mut effect = prepared(params);
        let mut loud = tone(1.0, 2048);
        effect.process(&mut loud);
        let mut quiet = tone(0.02, 1024);
        effect.process(&mut quiet);
        quiet
            .channel(0)
            .iter()
            .skip(48)
            .fold(0.0f32, |acc, value| acc.max(value.abs()))
    };
    let standard = measure(0);
    let alr = measure(1);
    assert!(
        alr > standard * 1.3,
        "the adaptive release did not recover sooner: {alr} against {standard}"
    );
}

#[test]
fn the_stereo_link_blends_the_channels() {
    let params = ParamSet::new(specs());
    params.set_f32("limit", -6.0).unwrap();
    params.set_f32("stereo_link", 0.0).unwrap();
    params.set_f32("lookahead", 1.0).unwrap();
    let mut effect = prepared(params);
    let mut buffer = AudioBuffer::new(2, 2048);
    buffer.configure(2, 2048).unwrap();
    for (index, sample) in buffer.channel_mut(0).iter_mut().enumerate() {
        *sample = 0.9 * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
    }
    for (index, sample) in buffer.channel_mut(1).iter_mut().enumerate() {
        *sample = 0.01 * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
    }
    effect.process(&mut buffer);
    let left = buffer
        .channel(0)
        .iter()
        .skip(64)
        .fold(0.0f32, |a, b| a.max(b.abs()));
    let right = buffer
        .channel(1)
        .iter()
        .skip(64)
        .fold(0.0f32, |a, b| a.max(b.abs()));
    assert!(left < 0.51, "the loud channel was not limited: {left}");
    assert!(
        right > 0.009,
        "the quiet channel was limited with it: {right}"
    );
}

#[path = "quality.rs"]
mod quality;
