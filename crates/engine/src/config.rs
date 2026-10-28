pub const SINK_NODE_NAME: &str = "pipewire-effects-sink";
pub const SINK_NODE_DESCRIPTION: &str = "PipeWire Effects Sink";
pub const GROUP_NAME: &str = "pipewire-effects";
pub const MAX_EFFECTS: usize = 16;
pub const MAX_QUANTUM_FRAMES: usize = 8_192;
pub const RING_SECONDS: f32 = 0.5;
pub const MAX_RATE_FOR_RING: u32 = 192_000;
pub const PROCESS_TIMEOUT_MS: u64 = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct EngineConfig {
    pub target: String,
    pub sink_node_name: String,
    pub sink_description: String,
    pub channels: usize,
    pub bypassed: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            target: String::new(),
            sink_node_name: SINK_NODE_NAME.to_owned(),
            sink_description: SINK_NODE_DESCRIPTION.to_owned(),
            channels: 2,
            bypassed: false,
        }
    }
}

impl EngineConfig {
    pub fn ring_capacity_samples(&self) -> usize {
        let seconds = RING_SECONDS.max(0.05);
        (MAX_RATE_FOR_RING as f32 * seconds) as usize * self.channels.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_holds_half_a_second_of_stereo_audio() {
        let config = EngineConfig::default();
        assert_eq!(config.ring_capacity_samples(), 192_000 * 2 / 2);
    }
}
