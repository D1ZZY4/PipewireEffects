use std::time::Duration;

use pipewire_effects_core::AudioFormat;

pub const NODE_GROUP: &str = "pipewire-effects";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    pub format: AudioFormat,

    pub capture_node_name: String,
    pub capture_node_description: String,
    pub playback_node_name: String,

    pub target_sink: Option<String>,

    pub ring_capacity: Duration,

    pub target_latency: Duration,

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
    #[must_use]
    pub fn target_fill_samples(&self) -> usize {
        let rate = f64::from(self.format.sample_rate);
        let samples = rate * self.target_latency.as_secs_f64() * f64::from(self.format.channels);
        (samples.ceil() as usize).max(1_024)
    }

    #[must_use]
    pub fn ring_capacity_samples(&self) -> usize {
        let rate = f64::from(self.format.sample_rate);
        let samples = rate * self.ring_capacity.as_secs_f64() * f64::from(self.format.channels);
        (samples.ceil() as usize).max(self.target_fill_samples() * 4)
    }

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
mod tests;
