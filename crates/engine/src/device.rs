use pipewire as pw;
use pw::spa::utils::dict::DictRef;
use pw::types::ObjectType;
use pwe_core::PipelineKind;
use std::cell::RefCell;
use std::rc::Rc;

use crate::error::EngineResult;

pub const MEDIA_CLASS_SINK: &str = "Audio/Sink";
pub const MEDIA_CLASS_SOURCE: &str = "Audio/Source";
pub const MEDIA_CLASS_DUPLEX: &str = "Audio/Duplex";
pub const VIRTUAL_SINK: &str = "Audio/Sink/Virtual";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: u32,
    pub serial: u64,
    pub name: String,
    pub description: String,
    pub media_class: String,
    pub virtual_device: bool,
    pub priority: i64,
}

impl DeviceInfo {
    pub fn from_properties(id: u32, props: &DictRef) -> Option<Self> {
        let media_class = props.get("media.class")?;
        if !is_device_class(media_class) {
            return None;
        }
        let name = props
            .get("node.name")
            .or_else(|| props.get("object.serial"))
            .unwrap_or("unnamed")
            .to_owned();
        let description = props
            .get("node.description")
            .or_else(|| props.get("device.description"))
            .or_else(|| props.get("node.nick"))
            .unwrap_or(&name)
            .to_owned();
        let virtual_device = props
            .get("node.virtual")
            .is_some_and(|value| value == "true");
        let priority = props
            .get("priority.session")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let serial = props
            .get("object.serial")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        Some(Self {
            id,
            serial,
            name,
            description,
            media_class: media_class.to_owned(),
            virtual_device,
            priority,
        })
    }

    pub fn is_sink(&self) -> bool {
        matches!(
            self.media_class.as_str(),
            MEDIA_CLASS_SINK | MEDIA_CLASS_DUPLEX | VIRTUAL_SINK
        )
    }

    pub fn is_source(&self) -> bool {
        matches!(
            self.media_class.as_str(),
            MEDIA_CLASS_SOURCE | MEDIA_CLASS_DUPLEX
        )
    }
}

pub fn is_device_class(media_class: &str) -> bool {
    matches!(
        media_class,
        MEDIA_CLASS_SINK | MEDIA_CLASS_SOURCE | MEDIA_CLASS_DUPLEX | VIRTUAL_SINK
    )
}

pub fn pick_sink<'a>(devices: &'a [DeviceInfo], own: &[&str]) -> Option<&'a DeviceInfo> {
    devices
        .iter()
        .filter(|device| device.is_sink() && !own.contains(&device.name.as_str()))
        .max_by_key(|device| (std::cmp::Reverse(device.virtual_device), device.priority))
}

pub fn pick_source<'a>(devices: &'a [DeviceInfo], own: &[&str]) -> Option<&'a DeviceInfo> {
    devices
        .iter()
        .filter(|device| device.is_source() && !own.contains(&device.name.as_str()))
        .max_by_key(|device| (std::cmp::Reverse(device.virtual_device), device.priority))
}

pub fn resolve<'a>(
    devices: &'a [DeviceInfo],
    target: &str,
    own: &[&str],
    kind: PipelineKind,
) -> Option<&'a DeviceInfo> {
    if target.is_empty() || target == "default" {
        return match kind {
            PipelineKind::Output => pick_sink(devices, own),
            PipelineKind::Input => pick_source(devices, own),
        };
    }
    devices
        .iter()
        .find(|device| device.name == target && !own.contains(&device.name.as_str()))
}

pub fn list_devices() -> EngineResult<Vec<DeviceInfo>> {
    pw::init();
    let main_loop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&main_loop, None)?;
    let core = context.connect_rc(None)?;
    let registry = core.get_registry_rc()?;

    let devices = Rc::new(RefCell::new(Vec::new()));
    let collected = Rc::clone(&devices);
    let listener = registry
        .add_listener_local()
        .global(move |global| {
            if global.type_ != ObjectType::Node {
                return;
            }
            if let Some(props) = global.props
                && let Some(device) = DeviceInfo::from_properties(global.id, props)
            {
                let mut known = collected.borrow_mut();
                if !known
                    .iter()
                    .any(|existing: &DeviceInfo| existing.name == device.name)
                {
                    known.push(device);
                }
            }
        })
        .register();

    let mut result = Vec::new();
    if crate::connection::roundtrip(&main_loop, &core).is_ok() {
        result = devices.borrow().clone();
    }
    let _ = listener;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn properties(entries: &[(&str, &str)]) -> pw::properties::PropertiesBox {
        let mut properties = pw::properties::PropertiesBox::new();
        let map: HashMap<&str, &str> = entries.iter().copied().collect();
        for (key, value) in map {
            properties.insert(key, value);
        }
        properties
    }

    #[test]
    fn only_device_nodes_are_recorded() {
        let node = properties(&[("media.class", "Audio/Sink"), ("node.name", "alsa_output")]);
        let stream = properties(&[("media.class", "Stream/Output/Audio")]);
        assert!(DeviceInfo::from_properties(1, node.as_ref()).is_some());
        assert!(DeviceInfo::from_properties(2, stream.as_ref()).is_none());
    }

    #[test]
    fn the_default_sink_prefers_hardware_with_the_highest_priority() {
        let devices = vec![
            DeviceInfo {
                serial: 0,
                id: 1,
                name: "virtual".into(),
                description: "Virtual".into(),
                media_class: MEDIA_CLASS_SINK.into(),
                virtual_device: true,
                priority: 900,
            },
            DeviceInfo {
                serial: 0,
                id: 2,
                name: "usb".into(),
                description: "USB".into(),
                media_class: MEDIA_CLASS_SINK.into(),
                virtual_device: false,
                priority: 500,
            },
            DeviceInfo {
                serial: 0,
                id: 3,
                name: "hdmi".into(),
                description: "HDMI".into(),
                media_class: MEDIA_CLASS_SINK.into(),
                virtual_device: false,
                priority: 800,
            },
        ];
        assert_eq!(
            pick_sink(&devices, &["pipewire-effects-sink"]).map(|device| device.name.as_str()),
            Some("hdmi")
        );
    }

    #[test]
    fn a_named_target_is_resolved_exactly() {
        let devices = vec![DeviceInfo {
            serial: 0,
            id: 1,
            name: "usb".into(),
            description: "USB".into(),
            media_class: MEDIA_CLASS_SINK.into(),
            virtual_device: false,
            priority: 100,
        }];
        assert_eq!(
            resolve(&devices, "usb", &[], PipelineKind::Output).map(|device| device.id),
            Some(1)
        );
        assert!(resolve(&devices, "missing", &[], PipelineKind::Output).is_none());
    }

    #[test]
    fn the_internal_nodes_are_never_chosen_as_a_destination() {
        let devices = vec![DeviceInfo {
            serial: 0,
            id: 1,
            name: "pipewire-effects-source".into(),
            description: "Internal".into(),
            media_class: MEDIA_CLASS_SOURCE.into(),
            virtual_device: true,
            priority: 1000,
        }];
        assert!(pick_source(&devices, &["pipewire-effects-source"]).is_none());
        assert_eq!(
            resolve(
                &devices,
                "",
                &["pipewire-effects-source"],
                PipelineKind::Input
            )
            .map(|d| d.id),
            None
        );
    }

    #[test]
    fn the_input_pipeline_picks_a_source_and_the_output_pipeline_a_sink() {
        let devices = vec![
            DeviceInfo {
                serial: 0,
                id: 1,
                name: "sink".into(),
                description: "Sink".into(),
                media_class: MEDIA_CLASS_SINK.into(),
                virtual_device: false,
                priority: 500,
            },
            DeviceInfo {
                serial: 0,
                id: 2,
                name: "source".into(),
                description: "Source".into(),
                media_class: MEDIA_CLASS_SOURCE.into(),
                virtual_device: false,
                priority: 500,
            },
        ];
        assert_eq!(
            resolve(&devices, "", &[], PipelineKind::Output).map(|d| d.name.as_str()),
            Some("sink")
        );
        assert_eq!(
            resolve(&devices, "", &[], PipelineKind::Input).map(|d| d.name.as_str()),
            Some("source")
        );
    }

    #[test]
    fn sources_and_sinks_are_separated() {
        let sink = DeviceInfo {
            serial: 0,
            id: 1,
            name: "a".into(),
            description: "a".into(),
            media_class: MEDIA_CLASS_SINK.into(),
            virtual_device: false,
            priority: 0,
        };
        let source = DeviceInfo {
            serial: 0,
            id: 2,
            name: "b".into(),
            description: "b".into(),
            media_class: MEDIA_CLASS_SOURCE.into(),
            virtual_device: false,
            priority: 0,
        };
        assert!(sink.is_sink() && !sink.is_source());
        assert!(source.is_source() && !source.is_sink());
    }
}
