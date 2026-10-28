//! Audio format and buffer primitives shared by every layer.
//!
//! Everything in PipewireEffects works on **interleaved `f32`** samples. That is
//! not an accident: it is the format PipeWire hands us when a stream is
//! negotiated as `F32LE`, and keeping one representation end to end removes
//! conversion passes from the realtime path.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A single audio sample. Processing is always done in 32-bit float.
pub type Sample = f32;

/// The channel arrangement of a stream.
///
/// The layout is derived from the channel count. We only need to distinguish
/// the cases that change how an effect behaves (mono needs a channel-summing
/// rule, stereo is the common case); everything else is treated as discrete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChannelLayout {
    /// A single channel.
    Mono,
    /// Two channels, conventionally left and right.
    Stereo,
    /// Six channels in the standard FL, FR, FC, LFE, RL, RR order.
    Surround51,
    /// Any other channel count, processed channel by channel.
    Discrete,
}

impl ChannelLayout {
    /// Derives the layout for a channel count.
    #[must_use]
    pub const fn from_channels(channels: u16) -> Self {
        match channels {
            1 => Self::Mono,
            2 => Self::Stereo,
            6 => Self::Surround51,
            _ => Self::Discrete,
        }
    }

    /// The canonical channel count for this layout.
    #[must_use]
    pub const fn channel_count(self) -> u16 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround51 => 6,
            Self::Discrete => 0,
        }
    }

    /// A short human readable name, used by the interface and by logs.
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

/// The negotiated format of an audio stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioFormat {
    /// Sample rate in Hertz.
    pub sample_rate: u32,
    /// Number of interleaved channels.
    pub channels: u16,
    /// Channel arrangement derived from `channels`.
    pub layout: ChannelLayout,
}

impl AudioFormat {
    /// Builds a format, deriving the layout from the channel count.
    #[must_use]
    pub const fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
            layout: ChannelLayout::from_channels(channels),
        }
    }

    /// 48 kHz stereo, the format we ask PipeWire to negotiate.
    #[must_use]
    pub const fn default_stereo() -> Self {
        Self::new(48_000, 2)
    }

    /// The sample rate as a float, for filter design maths.
    #[must_use]
    pub fn sample_rate_f32(self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        {
            self.sample_rate as f32
        }
    }

    /// The channel count as a `usize`, for indexing.
    #[must_use]
    pub const fn channels_usize(self) -> usize {
        self.channels as usize
    }

    /// Whether the format describes something we are able to process.
    ///
    /// The bounds are deliberately wide; the point is to reject zero, and to
    /// reject the absurd values that appear when a format has not been
    /// negotiated yet.
    #[must_use]
    pub const fn is_plausible(self) -> bool {
        self.sample_rate >= 8_000
            && self.sample_rate <= 768_000
            && self.channels >= 1
            && self.channels <= 64
    }

    /// Converts a sample count into a frame count.
    ///
    /// A zero channel count would divide by zero, so it yields zero frames
    /// instead. The `match` is used rather than `unwrap_or` because this is a
    /// `const fn` and `Option::unwrap_or` is not const-stable yet.
    #[must_use]
    pub const fn frames_of(self, samples: usize) -> usize {
        match samples.checked_div(self.channels as usize) {
            Some(frames) => frames,
            None => 0,
        }
    }

    /// Converts a frame count into a sample count.
    #[must_use]
    pub const fn samples_of(self, frames: usize) -> usize {
        frames * self.channels as usize
    }

    /// The wall-clock duration of a frame count.
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

/// A mutable view over one block of interleaved audio.
///
/// The underlying slice is usually a PipeWire buffer, which can be larger than
/// the amount of audio we are dealing with in a given cycle. The block
/// therefore tracks an explicit frame count; effects must go through
/// [`AudioBlock::active_samples`] or [`AudioBlock::frames_iter_mut`] so that
/// they never touch the tail of the allocation.
#[derive(Debug)]
pub struct AudioBlock<'a> {
    samples: &'a mut [Sample],
    frames: usize,
    format: AudioFormat,
}

impl<'a> AudioBlock<'a> {
    /// Wraps a buffer whose length is exactly the audio we want to process.
    #[must_use]
    pub fn new(samples: &'a mut [Sample], format: AudioFormat) -> Self {
        let frames = format.frames_of(samples.len());
        Self {
            samples,
            frames,
            format,
        }
    }

    /// Wraps a buffer that may be longer than the audio we want to process.
    ///
    /// `frames` is clamped so that `frames * channels` never exceeds the
    /// backing slice.
    #[must_use]
    pub fn with_frames(samples: &'a mut [Sample], format: AudioFormat, frames: usize) -> Self {
        let channels = format.channels_usize().max(1);
        let max_frames = samples.len() / channels;
        Self {
            samples,
            frames: frames.min(max_frames),
            format,
        }
    }

    /// The format of the block.
    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    /// The negotiated sample rate in Hertz.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.format.sample_rate
    }

    /// The number of channels.
    #[must_use]
    pub const fn channels(&self) -> usize {
        self.format.channels as usize
    }

    /// The number of frames in the block.
    #[must_use]
    pub const fn frames(&self) -> usize {
        self.frames
    }

    /// The entire backing slice, including any unused tail.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        self.samples
    }

    /// The entire backing slice, including any unused tail.
    pub fn samples_mut(&mut self) -> &mut [Sample] {
        self.samples
    }

    /// The samples that actually carry audio, i.e. `frames * channels` values.
    #[must_use]
    pub fn active_samples(&self) -> &[Sample] {
        let len = self.format.samples_of(self.frames).min(self.samples.len());
        &self.samples[..len]
    }

    /// The samples that actually carry audio, i.e. `frames * channels` values.
    pub fn active_samples_mut(&mut self) -> &mut [Sample] {
        let len = self.format.samples_of(self.frames).min(self.samples.len());
        &mut self.samples[..len]
    }

    /// Iterates over the frames of the block as per-frame slices.
    pub fn frames_iter(&self) -> std::slice::ChunksExact<'_, Sample> {
        let channels = self.channels().max(1);
        self.active_samples().chunks_exact(channels)
    }

    /// Iterates mutably over the frames of the block as per-frame slices.
    pub fn frames_iter_mut(&mut self) -> std::slice::ChunksExactMut<'_, Sample> {
        let channels = self.channels().max(1);
        self.active_samples_mut().chunks_exact_mut(channels)
    }

    /// Writes silence over the active region.
    pub fn fill_silence(&mut self) {
        self.active_samples_mut().fill(0.0);
    }

    /// Multiplies every active sample by `factor`.
    pub fn apply_gain(&mut self, factor: f32) {
        for sample in self.active_samples_mut() {
            *sample *= factor;
        }
    }

    /// The largest absolute sample value in the block.
    #[must_use]
    pub fn peak(&self) -> f32 {
        self.active_samples()
            .iter()
            .fold(0.0_f32, |acc, s| acc.max(s.abs()))
    }

    /// The root-mean-square level of the block.
    #[must_use]
    pub fn rms(&self) -> f32 {
        let samples = self.active_samples();
        if samples.is_empty() {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let count = samples.len() as f32;
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        (sum / count).sqrt()
    }

    /// Whether every active sample is zero.
    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.active_samples().iter().all(|s| *s == 0.0)
    }

    /// Replaces any non-finite sample with zero.
    ///
    /// A single `NaN` produced by a misbehaving filter would otherwise spread
    /// through the rest of the chain and silence the output forever.
    pub fn sanitise(&mut self) {
        for sample in self.active_samples_mut() {
            if !sample.is_finite() {
                *sample = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_derived_from_channel_count() {
        assert_eq!(ChannelLayout::from_channels(1), ChannelLayout::Mono);
        assert_eq!(ChannelLayout::from_channels(2), ChannelLayout::Stereo);
        assert_eq!(
            AudioFormat::new(48_000, 6).layout,
            ChannelLayout::Surround51
        );
        assert_eq!(AudioFormat::new(48_000, 4).layout, ChannelLayout::Discrete);
    }

    #[test]
    fn with_frames_clamps_to_the_backing_slice() {
        let mut buffer = [0.0_f32; 8];
        let block = AudioBlock::with_frames(&mut buffer, AudioFormat::new(48_000, 2), 99);
        assert_eq!(block.frames(), 4);
        assert_eq!(block.active_samples().len(), 8);
    }

    #[test]
    fn new_derives_the_frame_count() {
        let mut buffer = [0.0_f32; 10];
        let block = AudioBlock::new(&mut buffer, AudioFormat::new(48_000, 2));
        assert_eq!(block.frames(), 5);
    }

    #[test]
    fn peak_and_rms_are_computed_over_the_active_region_only() {
        let mut buffer = [0.0_f32; 4];
        buffer[0] = 0.5;
        buffer[1] = -0.5;
        let block = AudioBlock::with_frames(&mut buffer, AudioFormat::new(48_000, 2), 1);
        assert!((block.peak() - 0.5).abs() < f32::EPSILON);
        assert!((block.rms() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sanitise_removes_non_finite_samples() {
        let mut buffer = [f32::NAN, 1.0, f32::INFINITY, -1.0];
        let mut block = AudioBlock::new(&mut buffer, AudioFormat::new(48_000, 2));
        block.sanitise();
        assert_eq!(block.active_samples(), &[0.0, 1.0, 0.0, -1.0]);
    }

    #[test]
    fn implausible_formats_are_rejected() {
        assert!(!AudioFormat::new(0, 2).is_plausible());
        assert!(!AudioFormat::new(48_000, 0).is_plausible());
        assert!(AudioFormat::default_stereo().is_plausible());
    }
}
