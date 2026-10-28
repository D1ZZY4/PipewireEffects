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

pub fn apply_gains(buffer: &mut AudioBuffer, gains: &[f32; MAX_CHANNELS]) {
    for (index, channel) in buffer.channels_mut().enumerate() {
        let gain = gains.get(index).copied().unwrap_or(1.0);
        if (gain - 1.0).abs() < f32::EPSILON {
            continue;
        }
        for sample in channel.iter_mut() {
            *sample = math::sanitize(*sample * gain);
        }
    }
}

pub fn linked_peak(buffer: &AudioBuffer) -> f32 {
    let mut peak = 0.0f32;
    for index in 0..buffer.channels().min(MAX_CHANNELS) {
        for sample in buffer.channel(index) {
            peak = peak.max(sample.abs());
        }
    }
    peak
}

pub fn channel_peaks(buffer: &AudioBuffer) -> [f32; MAX_CHANNELS] {
    let mut peaks = [0.0f32; MAX_CHANNELS];
    for (index, slot) in peaks
        .iter_mut()
        .enumerate()
        .take(buffer.channels().min(MAX_CHANNELS))
    {
        let mut peak = 0.0f32;
        for sample in buffer.channel(index) {
            peak = peak.max(sample.abs());
        }
        *slot = peak;
    }
    peaks
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

pub fn soft_clip(sample: f32, ceiling: f32) -> f32 {
    if sample > ceiling {
        ceiling
    } else if sample < -ceiling {
        -ceiling
    } else {
        sample
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
    fn linked_peak_takes_the_loudest_channel() {
        let buffer = stereo([0.2, -0.8]);
        assert!((linked_peak(&buffer) - 0.8).abs() < 1.0e-6);
        assert_eq!(channel_peaks(&buffer)[1], 0.8);
    }

    #[test]
    fn gain_scales_every_channel() {
        let mut buffer = stereo([1.0, -1.0]);
        apply_gain(&mut buffer, 0.5);
        assert_eq!(buffer.channel(0)[0], 0.5);
        assert_eq!(buffer.channel(1)[0], -0.5);
    }

    #[test]
    fn soft_clip_respects_the_ceiling() {
        assert_eq!(soft_clip(2.0, 0.9), 0.9);
        assert_eq!(soft_clip(-2.0, 0.9), -0.9);
        assert_eq!(soft_clip(0.1, 0.9), 0.1);
    }
}
