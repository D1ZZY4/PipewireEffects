use super::*;

const RATE: u32 = 48_000;

fn tone(frequency: f32, amplitude: f32, frames: usize, offset: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = std::f32::consts::TAU * frequency * (offset + index) as f32 / RATE as f32;
            *sample = amplitude * phase.sin();
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

fn measured_gain(frequency: f32, settings: &[(&str, f32)]) -> f32 {
    let params = ParamSet::new(specs());
    for (id, value) in settings {
        params.set_f32(id, *value).unwrap();
    }
    let mut effect = LoudnessEffect::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    let frames = 4_800;
    let mut input = 0.0;
    let mut output = 0.0;
    for block in 0..20 {
        let mut buffer = tone(frequency, 0.25, frames, block * frames);
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
fn a_full_volume_leaves_the_contour_flat() {
    for frequency in [60.0, 1_000.0, 12_000.0] {
        let gain = measured_gain(frequency, &[("volume", 0.0)]);
        assert!(gain.abs() < 0.5, "{frequency} Hz read {gain} dB");
    }
}

#[test]
fn a_quiet_volume_lifts_the_bass_above_the_midrange() {
    let bass = measured_gain(60.0, &[("volume", -60.0)]);
    let middle = measured_gain(1_000.0, &[("volume", -60.0)]);
    assert!(bass > middle + 3.0, "bass {bass}, middle {middle}");
}

#[test]
fn the_flat_standard_disables_the_contour() {
    let bass = measured_gain(60.0, &[("standard", 0.0), ("volume", -60.0)]);
    assert!(bass.abs() < 0.5, "{bass}");
}

#[test]
fn the_standards_differ_in_how_far_they_lift_the_bass() {
    let iso = measured_gain(60.0, &[("standard", 1.0), ("volume", -60.0)]);
    let fletcher = measured_gain(60.0, &[("standard", 2.0), ("volume", -60.0)]);
    assert!(fletcher > iso + 1.0, "fletcher {fletcher}, iso {iso}");
}

#[test]
fn clipping_holds_the_ceiling() {
    let params = ParamSet::new(specs());
    params.set_bool("clipping", true).unwrap();
    params.set_f32("clipping_range", 6.0).unwrap();
    let mut effect = LoudnessEffect::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    let ceiling = math::db_to_gain(-6.0);
    for block in 0..8 {
        let mut buffer = tone(80.0, 1.0, 4_800, block * 4_800);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.abs() <= ceiling + 1.0e-4, "{}", sample.abs());
            }
        }
    }
}

#[test]
fn the_output_stays_finite_at_the_extremes() {
    let params = ParamSet::new(specs());
    params.set_f32("volume", -83.0).unwrap();
    params.set_bool("clipping", true).unwrap();
    params.set_f32("clipping_range", 0.0).unwrap();
    let mut effect = LoudnessEffect::new(params);
    effect.prepare(AudioFormat::new(RATE, 2));
    for block in 0..8 {
        let mut buffer = tone(40.0, 2.0, 4_800, block * 4_800);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(
                    sample.is_finite() && sample.abs() <= 1.0 + 1.0e-4,
                    "{sample}"
                );
            }
        }
    }
}
