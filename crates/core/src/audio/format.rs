use std::fmt;

use serde::{Deserialize, Serialize};

pub type Sample = f32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround51,
    Discrete,
}

impl ChannelLayout {
    #[must_use]
    pub const fn from_channels(channels: u16) -> Self {
        match channels {
            1 => Self::Mono,
            2 => Self::Stereo,
            6 => Self::Surround51,
            _ => Self::Discrete,
        }
    }

    #[must_use]
    pub const fn channel_count(self) -> u16 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround51 => 6,
            Self::Discrete => 0,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mono => "Mono",
            Self::Stereo => "Stereo",
            Self::Surround51 => "5.1",
            Self::Discrete => "Discrete",
        }
    }
}

impl fmt::Display for ChannelLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub layout: ChannelLayout,
}

impl AudioFormat {
    #[must_use]
    pub const fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
            layout: ChannelLayout::from_channels(channels),
        }
    }

    #[must_use]
    pub const fn default_stereo() -> Self {
        Self::new(48_000, 2)
    }

    #[must_use]
    pub fn sample_rate_f32(self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        {
            self.sample_rate as f32
        }
    }

    #[must_use]
    pub const fn channels_usize(self) -> usize {
        self.channels as usize
    }

    #[must_use]
    pub const fn is_plausible(self) -> bool {
        self.sample_rate >= 8_000
            && self.sample_rate <= 768_000
            && self.channels >= 1
            && self.channels <= 64
    }

    #[must_use]
    pub const fn frames_of(self, samples: usize) -> usize {
        match samples.checked_div(self.channels as usize) {
            Some(frames) => frames,
            None => 0,
        }
    }

    #[must_use]
    pub const fn samples_of(self, frames: usize) -> usize {
        frames * self.channels as usize
    }

    #[must_use]
    pub fn duration_seconds(self, frames: usize) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            frames as f64 / f64::from(self.sample_rate)
        }
    }
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::default_stereo()
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} Hz, {} ({} ch)",
            self.sample_rate, self.layout, self.channels
        )
    }
}

#[cfg(test)]
mod tests;
