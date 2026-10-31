use pwe_core::AudioBuffer;

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

fn feed(loudness: &mut Loudness, amplitude: f32, hops: usize) {
    for _ in 0..hops {
        loudness.process(&tone(amplitude, 4_800));
    }
}

#[test]
fn the_momentary_window_needs_four_hops() {
    let mut loudness = Loudness::new(48_000);
    feed(&mut loudness, 0.5, 3);
    assert!(loudness.momentary().is_none());
    feed(&mut loudness, 0.5, 1);
    assert!(loudness.momentary().is_some());
}

#[test]
fn the_short_term_window_needs_three_seconds() {
    let mut loudness = Loudness::new(48_000);
    feed(&mut loudness, 0.5, 29);
    assert!(loudness.short_term().is_none());
    feed(&mut loudness, 0.5, 1);
    assert!(loudness.short_term().is_some());
}

#[test]
fn the_windows_agree_on_a_steady_tone() {
    let mut loudness = Loudness::new(48_000);
    feed(&mut loudness, 0.5, 40);
    let momentary = loudness.momentary().expect("momentary");
    let short_term = loudness.short_term().expect("short term");
    let integrated = loudness.integrated().expect("integrated");
    assert!(
        (momentary - short_term).abs() < 0.3,
        "{momentary} {short_term}"
    );
    assert!(
        (short_term - integrated).abs() < 0.3,
        "{short_term} {integrated}"
    );
}
