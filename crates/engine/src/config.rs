use pwe_core::PipelineKind;

pub const OUTPUT_NODE_NAME: &str = "pipewire-effects-sink";
pub const OUTPUT_NODE_DESCRIPTION: &str = "PipeWire Effects Sink";
pub const INPUT_NODE_NAME: &str = "pipewire-effects-source";
pub const INPUT_NODE_DESCRIPTION: &str = "PipeWire Effects Source";
pub const OUTPUT_STREAM_NODE: &str = "pipewire-effects-output";
pub const INPUT_STREAM_NODE: &str = "pipewire-effects-input";
pub const GROUP_NAME: &str = "pipewire-effects";
pub const AUTO_TARGET: &str = "default";
pub const MAX_EFFECTS: usize = 16;
pub const MAX_QUANTUM_FRAMES: usize = 8_192;
pub const RING_SECONDS: f32 = 0.5;
pub const MAX_RATE_FOR_RING: u32 = 192_000;
pub const PROCESS_TIMEOUT_MS: u64 = 20;
pub const RETRY_BACKOFF_MS: u64 = 1_500;

pub const OUTPUT_METADATA_KEY: &str = "default.audio.sink";
pub const INPUT_METADATA_KEY: &str = "default.audio.source";

pub fn virtual_node_name(kind: PipelineKind) -> &'static str {
    match kind {
        PipelineKind::Output => OUTPUT_NODE_NAME,
        PipelineKind::Input => INPUT_NODE_NAME,
    }
}

pub fn virtual_node_description(kind: PipelineKind) -> &'static str {
    pwe_core::i18n::t(match kind {
        PipelineKind::Output => OUTPUT_NODE_DESCRIPTION,
        PipelineKind::Input => INPUT_NODE_DESCRIPTION,
    })
}

pub fn virtual_media_class(kind: PipelineKind) -> &'static str {
    match kind {
        PipelineKind::Output => "Audio/Sink",
        PipelineKind::Input => "Audio/Source",
    }
}

pub fn stream_node_name(kind: PipelineKind) -> &'static str {
    match kind {
        PipelineKind::Output => OUTPUT_STREAM_NODE,
        PipelineKind::Input => INPUT_STREAM_NODE,
    }
}

pub fn metadata_key(kind: PipelineKind) -> &'static str {
    match kind {
        PipelineKind::Output => OUTPUT_METADATA_KEY,
        PipelineKind::Input => INPUT_METADATA_KEY,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineConfig {
    pub kind: PipelineKind,
    pub target: String,
    pub node_name: String,
    pub node_description: String,
    pub channels: usize,
    pub bypassed: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self::for_kind(PipelineKind::Output)
    }
}

impl EngineConfig {
    pub fn for_kind(kind: PipelineKind) -> Self {
        Self {
            kind,
            target: AUTO_TARGET.to_owned(),
            node_name: virtual_node_name(kind).to_owned(),
            node_description: virtual_node_description(kind).to_owned(),
            channels: 2,
            bypassed: false,
        }
    }

    pub fn own_nodes(&self) -> [&str; 2] {
        [self.node_name.as_str(), stream_node_name(self.kind)]
    }

    pub fn owned_nodes(&self) -> [String; 2] {
        [
            self.node_name.clone(),
            stream_node_name(self.kind).to_owned(),
        ]
    }

    pub fn follows_the_system_default(&self) -> bool {
        self.target.is_empty() || self.target == AUTO_TARGET
    }

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

    #[test]
    fn each_pipeline_has_its_own_nodes_and_metadata_key() {
        let output = EngineConfig::for_kind(PipelineKind::Output);
        let input = EngineConfig::for_kind(PipelineKind::Input);
        assert_eq!(output.node_name, "pipewire-effects-sink");
        assert_eq!(input.node_name, "pipewire-effects-source");
        assert_ne!(output.own_nodes(), input.own_nodes());
        assert_eq!(metadata_key(PipelineKind::Input), "default.audio.source");
        assert_eq!(virtual_media_class(PipelineKind::Input), "Audio/Source");
    }

    #[test]
    fn a_fresh_configuration_follows_the_system_default() {
        assert!(EngineConfig::default().follows_the_system_default());
    }
}
