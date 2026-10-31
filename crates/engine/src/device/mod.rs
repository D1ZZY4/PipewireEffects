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
#[path = "tests/tests.rs"]
mod tests;
