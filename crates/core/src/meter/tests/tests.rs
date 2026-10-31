use super::*;

fn sine(frames: usize, amplitude: f32) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            let phase = std::f32::consts::TAU * 997.0 * index as f32 / 48_000.0;
            *sample = amplitude * phase.sin();
        }
    }
    buffer
}

#[test]
fn peak_and_rms_track_a_sine() {
    let mut meter = LevelMeter::new();
    meter.set_rate(48_000);
    for _ in 0..200 {
        meter.process(&sine(1024, 0.5));
    }
    assert!((meter.peak(0) - 0.5).abs() < 0.02, "peak {}", meter.peak(0));
    assert!((meter.rms(0) - 0.3535).abs() < 0.02, "rms {}", meter.rms(0));
    assert!((meter.peak_db(0) - -6.02).abs() < 0.5);
}

#[test]
fn silence_decays_to_the_floor() {
    let mut meter = LevelMeter::new();
    meter.set_rate(48_000);
    meter.process(&sine(4096, 1.0));
    let mut silent = AudioBuffer::new(2, 4096);
    silent.configure(2, 4096).unwrap();
    for _ in 0..2000 {
        meter.process(&silent);
    }
    assert!(meter.peak_db(0) < -100.0);
    assert!(meter.rms_db(0) < -100.0);
}
