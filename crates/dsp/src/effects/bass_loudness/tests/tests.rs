use super::*;

const RATE: u32 = 48_000;

fn tone(frequency: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = std::f32::consts::TAU * frequency * (offset + index) as f32 / RATE as f32;
            *sample = 0.25 * phase.sin();
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

fn measured_gain(frequency: f32, settings: [(&str, f32); 3]) -> f32 {
    let params = ParamSet::new(specs());
    for (id, value) in settings {
        params.set_f32(id, value).unwrap();
    }
    let mut effect = BassLoudness::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    let frames = 4800;
    let mut input = 0.0;
    let mut output = 0.0;
    for block in 0..20 {
        let mut buffer = tone(frequency, frames, block * frames);
        let before = energy(&buffer);
        effect.process(&mut buffer);
        if block >= 16 {
            input += before;
            output += energy(&buffer);
        }
    }
    (10.0 * (output / input).log10()) as f32
}

#[test]
fn a_quiet_source_lifts_the_bass_more_than_the_midrange() {
    let bass = measured_gain(60.0, [("loudness", -50.0), ("output", 0.0), ("link", 0.0)]);
    let middle = measured_gain(
        1_000.0,
        [("loudness", -50.0), ("output", 0.0), ("link", 0.0)],
    );
    assert!(bass > middle + 3.0, "bass {bass}, middle {middle}");
    assert!(bass < 18.5, "{bass}");
}

#[test]
fn a_full_level_source_keeps_the_contour_flat() {
    for frequency in [60.0, 1_000.0, 12_000.0] {
        let gain = measured_gain(
            frequency,
            [("loudness", 0.0), ("output", 0.0), ("link", 0.0)],
        );
        assert!(gain.abs() < 0.6, "{frequency} Hz read {gain} dB");
    }
}

#[test]
fn the_output_control_trims_the_level() {
    let gain = measured_gain(
        1_000.0,
        [("loudness", 0.0), ("output", -6.0), ("link", 0.0)],
    );
    assert!((gain + 6.0).abs() < 0.3, "{gain}");
}

#[test]
fn the_link_control_lowers_the_output_as_the_source_falls() {
    let linked = measured_gain(
        1_000.0,
        [("loudness", -50.0), ("output", 0.0), ("link", -9.1)],
    );
    let open = measured_gain(
        1_000.0,
        [("loudness", -50.0), ("output", 0.0), ("link", 0.0)],
    );
    assert!(linked < open - 4.0, "linked {linked}, open {open}");
}

#[test]
fn the_output_stays_finite_at_the_extremes() {
    let params = ParamSet::new(specs());
    params.set_f32("loudness", -60.0).unwrap();
    params.set_f32("output", -60.0).unwrap();
    params.set_f32("link", -60.0).unwrap();
    let mut effect = BassLoudness::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    for block in 0..4 {
        let mut buffer = tone(40.0, 4800, block * 4800);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 4.0, "{sample}");
            }
        }
    }
}
