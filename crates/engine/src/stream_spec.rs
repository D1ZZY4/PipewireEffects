use pipewire as pw;
use pw::stream::StreamFlags;
use pwe_core::PipelineKind;
use pwe_core::i18n::t;

use crate::config::{self, EngineConfig, GROUP_NAME};
use crate::streams::stream_properties;

pub struct NodeSpec {
    pub media_class: &'static str,
    pub category: &'static str,
    pub node_name: String,
    pub description: String,
    pub virtual_device: bool,
    pub target: String,
}

impl NodeSpec {
    pub fn properties(&self) -> pw::properties::PropertiesBox {
        let mut properties = stream_properties(
            self.media_class,
            self.category,
            "DSP",
            &self.node_name,
            &self.description,
            GROUP_NAME,
        );
        if self.virtual_device {
            properties.insert("node.virtual", "true");
            properties.insert("priority.session", "0");
            properties.insert("monitor.channel-volumes", "false");
        }
        if !self.target.is_empty() {
            properties.insert("target.object", self.target.as_str());
            properties.insert("node.target", self.target.as_str());
        }
        properties
    }

    pub fn flags(&self) -> StreamFlags {
        let base = StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS;
        if self.target.is_empty() {
            base
        } else {
            base | StreamFlags::AUTOCONNECT
        }
    }

    #[cfg(test)]
    pub fn autoconnects(&self) -> bool {
        !self.target.is_empty()
    }
}

pub fn upstream(config: &EngineConfig, target: &str) -> NodeSpec {
    match config.kind {
        PipelineKind::Output => NodeSpec {
            media_class: config::virtual_media_class(config.kind),
            category: "Capture",
            node_name: config.node_name.clone(),
            description: config.node_description.clone(),
            virtual_device: true,
            target: String::new(),
        },
        PipelineKind::Input => NodeSpec {
            media_class: "Stream/Input/Audio",
            category: "Capture",
            node_name: config::stream_node_name(config.kind).to_owned(),
            description: t("PipeWire Effects Input").to_owned(),
            virtual_device: false,
            target: target.to_owned(),
        },
    }
}

pub fn downstream(config: &EngineConfig, target: &str) -> NodeSpec {
    match config.kind {
        PipelineKind::Output => NodeSpec {
            media_class: "Stream/Output/Audio",
            category: "Playback",
            node_name: config::stream_node_name(config.kind).to_owned(),
            description: t("PipeWire Effects Output").to_owned(),
            virtual_device: false,
            target: target.to_owned(),
        },
        PipelineKind::Input => NodeSpec {
            media_class: config::virtual_media_class(config.kind),
            category: "Playback",
            node_name: config.node_name.clone(),
            description: config.node_description.clone(),
            virtual_device: true,
            target: String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_output_pipeline_offers_a_sink_and_plays_to_the_chosen_device() {
        let config = EngineConfig::for_kind(PipelineKind::Output);
        let up = upstream(&config, "speakers");
        let down = downstream(&config, "speakers");
        assert_eq!(up.media_class, "Audio/Sink");
        assert_eq!(up.node_name, "pipewire-effects-sink");
        assert!(up.virtual_device);
        assert!(up.target.is_empty());
        assert_eq!(down.media_class, "Stream/Output/Audio");
        assert_eq!(down.target, "speakers");
        assert!(!down.virtual_device);
    }

    #[test]
    fn the_input_pipeline_reads_the_chosen_device_and_offers_a_source() {
        let config = EngineConfig::for_kind(PipelineKind::Input);
        let up = upstream(&config, "mic");
        let down = downstream(&config, "mic");
        assert_eq!(up.media_class, "Stream/Input/Audio");
        assert_eq!(up.target, "mic");
        assert!(!up.virtual_device);
        assert_eq!(down.media_class, "Audio/Source");
        assert_eq!(down.node_name, "pipewire-effects-source");
        assert!(down.virtual_device);
        assert!(down.target.is_empty());
    }

    #[test]
    fn only_the_stream_bound_to_a_device_asks_to_be_autoconnected() {
        let output = EngineConfig::for_kind(PipelineKind::Output);
        assert!(!upstream(&output, "speakers").autoconnects());
        assert!(downstream(&output, "speakers").autoconnects());

        let input = EngineConfig::for_kind(PipelineKind::Input);
        assert!(upstream(&input, "mic").autoconnects());
        assert!(!downstream(&input, "mic").autoconnects());
    }

    #[test]
    fn a_virtual_node_never_carries_a_target() {
        for kind in pwe_core::ALL_PIPELINES {
            let config = EngineConfig::for_kind(kind);
            let spec = if kind == PipelineKind::Output {
                upstream(&config, "device")
            } else {
                downstream(&config, "device")
            };
            assert!(spec.virtual_device);
            assert!(spec.target.is_empty());
            assert!(!spec.autoconnects());
        }
    }
}
