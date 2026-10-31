use pwe_core::{AudioBuffer, AudioFormat};

use super::registry::{EFFECTS, instantiate};

fn tone(frames: usize) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(2, frames);
    buffer.configure(2, frames).unwrap();
    for channel in 0..2 {
        let plan = buffer.channel_mut(channel);
        for (index, sample) in plan.iter_mut().enumerate() {
            *sample = 0.2 * (std::f32::consts::TAU * 997.0 * index as f32 / 48_000.0).sin();
        }
    }
    buffer
}

#[test]
fn every_registered_effect_processes_a_tone_without_going_non_finite() {
    for descriptor in EFFECTS {
        let (_params, mut effect) =
            instantiate(descriptor.id).unwrap_or_else(|| panic!("{} did not build", descriptor.id));
        effect.prepare(AudioFormat::new(48_000, 2));
        for _ in 0..8 {
            let mut buffer = tone(1024);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite(), "{} produced {sample}", descriptor.id);
                }
            }
        }
    }
}
