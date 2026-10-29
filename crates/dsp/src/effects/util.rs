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
mod tests {
    use super::*;

    fn stereo(values: [f32; 2]) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, 2);
        buffer.configure(2, 1).unwrap();
        buffer.channel_mut(0)[0] = values[0];
        buffer.channel_mut(1)[0] = values[1];
        buffer
    }

    #[test]
    fn gain_scales_every_channel() {
        let mut buffer = stereo([1.0, -1.0]);
        apply_gain(&mut buffer, 0.5);
        assert_eq!(buffer.channel(0)[0], 0.5);
        assert_eq!(buffer.channel(1)[0], -0.5);
    }

    #[test]
    fn mean_square_matches_the_signal_power() {
        let buffer = stereo([0.5, -0.5]);
        assert!((mean_square(&buffer) - 0.25).abs() < 1.0e-6);
    }
}
