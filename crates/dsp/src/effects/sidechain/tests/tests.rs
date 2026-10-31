use super::*;
use pwe_core::AudioBuffer;

fn tone(hz: f32, amplitude: f32, frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(1, frames);
    buffer.configure(1, frames).unwrap();
    let plan = buffer.channel_mut(0);
    for (index, sample) in plan.iter_mut().enumerate() {
        *sample = amplitude * (std::f32::consts::TAU * hz * index as f32 / 48_000.0).sin();
    }
    buffer
}

fn measure(mode: usize, frequency: f32, hz: f32) -> f32 {
    let mut filter = SidechainFilter::new(48_000);
    filter.update(mode, frequency, 0, 20_000.0);
    let buffer = tone(hz, 0.5, 8_192);
    let mut peak = 0.0f32;
    for index in 0..8_192 {
        let value = filter.process(0, buffer.channel(0)[index]);
        if index >= 4_096 {
            peak = peak.max(value.abs());
        }
    }
    peak
}

#[test]
fn the_high_pass_removes_the_low_tone_below_its_cutoff() {
    let open = measure(0, 10.0, 60.0);
    let filtered = measure(1, 400.0, 60.0);
    assert!(open > 0.4, "{open}");
    assert!(filtered < open * 0.2, "{filtered} against {open}");
}

#[test]
fn a_steeper_high_pass_cut_more() {
    let one = measure(1, 400.0, 60.0);
    let three = measure(3, 400.0, 60.0);
    assert!(three < one, "{three} !< {one}");
}

#[test]
fn the_high_pass_leaves_the_passband_alone() {
    let open = measure(0, 10.0, 4_000.0);
    let filtered = measure(3, 400.0, 4_000.0);
    assert!((open - filtered).abs() < 0.05, "{open} against {filtered}");
}
