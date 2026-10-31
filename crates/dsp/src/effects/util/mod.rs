pub const MAX_CHANNELS: usize = 8;

use pwe_core::{AudioBuffer, math};

pub fn apply_gain(buffer: &mut AudioBuffer, gain: f32) {
    if (gain - 1.0).abs() < f32::EPSILON {
        return;
    }
    for channel in buffer.channels_mut() {
        for sample in channel.iter_mut() {
            *sample = math::sanitize(*sample * gain);
        }
    }
}

pub fn mean_square(buffer: &AudioBuffer) -> f32 {
    let mut sum = 0.0f64;
    let mut count = 0usize;
    for channel in 0..buffer.channels().min(MAX_CHANNELS) {
        for sample in buffer.channel(channel) {
            sum += f64::from(*sample) * f64::from(*sample);
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        (sum / count as f64) as f32
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
