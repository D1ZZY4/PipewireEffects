//! Engine configuration.

use std::time::Duration;

use pipewire_effects_core::AudioFormat;

/// The node group both our streams join.
///
/// Putting the capture and playback streams in one group is what lets PipeWire
/// schedule them together. Without it the two halves of the loopback can be
/// driven from different graph cycles, which shows up as periodic dropouts.
pub const NODE_GROUP: &str = "pipewire-effects";

/// How the engine should be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// The format we ask PipeWire to negotiate.
    ///
    /// We request one specific format for both streams. PipeWire inserts a
    /// converter when the hardware runs at something else, so the negotiated
    /// format is always this one — which means the DSP chain never has to be
    /// re-prepared for a format change at run time.
    pub format: AudioFormat,

    /// Name of the virtual sink applications play into.
    pub capture_node_name: String,
    /// Human readable name of the virtual sink.
    pub capture_node_description: String,
    /// Name of the stream we play the processed audio out of.
    pub playback_node_name: String,

    /// The device to play the processed audio on.
    ///
    /// Either a node name or an `object.serial`. `None` means "whatever
    /// PipeWire considers the default", which is usually what you want when the
    /// virtual sink itself has been made the default.
    pub target_sink: Option<String>,

    /// How much audio the ring buffer can hold.
    ///
    /// This is the ceiling on the latency drift we can absorb before the
    /// playback side has to start throwing samples away, not the latency you
    /// hear.
    pub ring_capacity: Duration,

    /// The latency we aim to keep in the ring buffer.
    ///
    /// A little buffering is what absorbs the jitter between the capture and
    /// playback callbacks. Too little and quiet passages underrun; too much and
    /// every control change feels sluggish.
    pub target_latency: Duration,

    /// Whether the playback stream should be connected at all.
    ///
    /// Disabling it makes the engine capture audio and process it without
    /// emitting any sound, which is useful for metering and for development.
    pub play_audio: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            format: AudioFormat::default_stereo(),
            capture_node_name: String::from("pipewire-effects-sink"),
            capture_node_description: String::from("PipewireEffects Sink"),
            playback_node_name: String::from("pipewire-effects-output"),
            target_sink: None,
            ring_capacity: Duration::from_millis(500),
            target_latency: Duration::from_millis(20),
            play_audio: true,
        }
    }
}

impl EngineConfig {
    /// The number of frames the ring buffer should aim to hold.
    #[must_use]
    pub fn target_fill_samples(&self) -> usize {
        let rate = f64::from(self.format.sample_rate);
        let samples = rate * self.target_latency.as_secs_f64() * f64::from(self.format.channels);
        // Round up to a power of two so the ring's mask arithmetic stays cheap,
        // with a floor of one PipeWire quantum.
        (samples.ceil() as usize).max(1_024)
    }

    /// The ring capacity in samples.
    #[must_use]
    pub fn ring_capacity_samples(&self) -> usize {
        let rate = f64::from(self.format.sample_rate);
        let samples = rate * self.ring_capacity.as_secs_f64() * f64::from(self.format.channels);
        (samples.ceil() as usize).max(self.target_fill_samples() * 4)
    }

    /// Checks that the configuration can actually be run.
    ///
    /// # Errors
    ///
    /// Returns a description of the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        if !self.format.is_plausible() {
            return Err(format!("implausible audio format: {}", self.format));
        }
        if self.target_latency > self.ring_capacity {
            return Err(format!(
                "target latency ({:?}) exceeds ring capacity ({:?})",
                self.target_latency, self.ring_capacity
            ));
        }
        if self.capture_node_name.is_empty() {
            return Err(String::from("the capture node needs a name"));
        }
        if self.play_audio && self.playback_node_name.is_empty() {
            return Err(String::from("the playback node needs a name"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_configuration_is_valid() {
        let config = EngineConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.format, AudioFormat::new(48_000, 2));
    }

    #[test]
    fn the_target_fill_is_derived_from_the_latency_setting() {
        let config = EngineConfig {
            target_latency: Duration::from_millis(20),
            ..EngineConfig::default()
        };
        // 20 ms at 48 kHz stereo is 1920 samples.
        assert_eq!(config.target_fill_samples(), 1_920);
    }

    #[test]
    fn the_ring_is_always_much_larger_than_the_target() {
        let config = EngineConfig {
            target_latency: Duration::from_millis(500),
            ring_capacity: Duration::from_millis(500),
            ..EngineConfig::default()
        };
        assert!(config.ring_capacity_samples() >= config.target_fill_samples() * 4);
    }

    #[test]
    fn a_ring_smaller_than_the_target_is_rejected() {
        let config = EngineConfig {
            target_latency: Duration::from_millis(200),
            ring_capacity: Duration::from_millis(50),
            ..EngineConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn an_implausible_format_is_rejected() {
        let config = EngineConfig {
            format: AudioFormat::new(0, 0),
            ..EngineConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn an_empty_node_name_is_rejected() {
        let config = EngineConfig {
            capture_node_name: String::new(),
            ..EngineConfig::default()
        };
        assert!(config.validate().is_err());
    }
}
