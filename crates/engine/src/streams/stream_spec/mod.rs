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
#[path = "tests/tests.rs"]
mod tests;
