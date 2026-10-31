use crate::error::{CoreError, CoreResult};
use crate::math::sanitize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioFormat {
    rate: u32,
    channels: usize,
}

impl AudioFormat {
    pub const fn new(rate: u32, channels: usize) -> Self {
        Self { rate, channels }
    }

    pub fn validated(rate: u32, channels: usize) -> CoreResult<Self> {
        if !(8_000..=768_000).contains(&rate) {
            return Err(CoreError::InvalidValue {
                id: "audio.rate".to_owned(),
                detail: format!("{rate} Hz is outside the supported range"),
            });
        }
        if !(1..=64).contains(&channels) {
            return Err(CoreError::InvalidValue {
                id: "audio.channels".to_owned(),
                detail: format!("{channels} channels is outside the supported range"),
            });
        }
        Ok(Self { rate, channels })
    }

    pub const fn rate(&self) -> u32 {
        self.rate
    }

    pub const fn channels(&self) -> usize {
        self.channels
    }
}

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    data: Vec<f32>,
    channels: usize,
    capacity_frames: usize,
    frames: usize,
}

impl AudioBuffer {
    pub fn new(channels: usize, capacity_frames: usize) -> Self {
        let channels = channels.max(1);
        let capacity_frames = capacity_frames.max(1);
        Self {
            data: vec![0.0; channels * capacity_frames],
            channels,
            capacity_frames,
            frames: 0,
        }
    }

    pub fn configure(&mut self, channels: usize, frames: usize) -> CoreResult<()> {
        if frames > self.capacity_frames {
            return Err(CoreError::BufferCapacity {
                needed: frames,
                available: self.capacity_frames,
            });
        }
        if channels > self.channels {
            return Err(CoreError::BufferCapacity {
                needed: channels * self.capacity_frames,
                available: self.data.len(),
            });
        }
        if channels != self.channels {
            self.data.fill(0.0);
            self.channels = channels;
        }
        self.frames = frames;
        Ok(())
    }

    pub const fn channels(&self) -> usize {
        self.channels
    }

    pub const fn frames(&self) -> usize {
        self.frames
    }

    pub const fn capacity_frames(&self) -> usize {
        self.capacity_frames
    }

    pub fn channel(&self, channel: usize) -> &[f32] {
        let start = channel * self.capacity_frames;
        &self.data[start..start + self.frames]
    }

    pub fn channel_mut(&mut self, channel: usize) -> &mut [f32] {
        let start = channel * self.capacity_frames;
        &mut self.data[start..start + self.frames]
    }

    pub fn channels_mut(&mut self) -> impl Iterator<Item = &mut [f32]> {
        let frames = self.frames;
        let capacity = self.capacity_frames;
        self.data
            .chunks_mut(capacity)
            .map(move |planar| &mut planar[..frames])
    }

    pub fn peak(&self) -> f32 {
        let mut peak = 0.0f32;
        for channel in self.active_channels() {
            for sample in channel {
                peak = peak.max(sample.abs());
            }
        }
        peak
    }

    pub fn rms(&self) -> f32 {
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for channel in self.active_channels() {
            for sample in channel {
                sum += f64::from(*sample) * f64::from(*sample);
                count += 1;
            }
        }
        if count == 0 {
            return 0.0;
        }
        (sum / count as f64).sqrt() as f32
    }

    pub fn sanitize(&mut self) {
        for sample in self.data.iter_mut() {
            *sample = sanitize(*sample);
        }
    }

    pub fn deinterleave_from(&mut self, source: &[f32]) -> usize {
        if self.channels == 0 {
            return 0;
        }
        let available = source.len() / self.channels;
        let frames = available.min(self.frames);
        for channel in 0..self.channels {
            let base = channel * self.capacity_frames;
            for frame in 0..frames {
                self.data[base + frame] = source[frame * self.channels + channel];
            }
        }
        frames
    }

    pub fn interleave_into(&self, destination: &mut [f32]) -> usize {
        if self.channels == 0 {
            return 0;
        }
        let frames = (destination.len() / self.channels).min(self.frames);
        for channel in 0..self.channels {
            let base = channel * self.capacity_frames;
            for frame in 0..frames {
                destination[frame * self.channels + channel] = self.data[base + frame];
            }
        }
        frames
    }

    fn active_channels(&self) -> impl Iterator<Item = &[f32]> {
        let frames = self.frames;
        let capacity = self.capacity_frames;
        self.data
            .chunks_exact(capacity)
            .map(move |planar| &planar[..frames])
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
