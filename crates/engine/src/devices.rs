use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use pipewire as pw;
use pw::spa;
use pw::types::ObjectType;

use crate::error::Result;

pub const DEFAULT_DISCOVERY_TIMEOUT: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: u32,
    pub serial: u64,
    pub name: String,
    pub description: String,
    pub nick: String,
    pub media_class: String,
}

impl AudioDevice {
    #[must_use]
    pub fn is_sink(&self) -> bool {
        self.media_class.starts_with("Audio/Sink")
    }

    #[must_use]
    pub fn is_source(&self) -> bool {
        self.media_class.starts_with("Audio/Source")
    }

    #[must_use]
    pub fn is_monitor(&self) -> bool {
        self.is_source() && self.name.ends_with(".monitor")
    }

    #[must_use]
    pub fn is_virtual_sink(&self) -> bool {
        self.name.starts_with("pipewire-effects")
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        if self.description.is_empty() {
            &self.name
        } else {
            &self.description
        }
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        if self.nick.is_empty() {
            &self.name
        } else {
            &self.nick
        }
    }

    fn from_properties(id: u32, properties: &spa::utils::dict::DictRef) -> Option<Self> {
        let media_class = properties.get("media.class")?;
        if !(media_class.starts_with("Audio/Sink") || media_class.starts_with("Audio/Source")) {
            return None;
        }
        Some(Self {
            id,
            serial: properties
                .get("object.serial")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            name: properties.get("node.name").unwrap_or_default().to_owned(),
            description: properties
                .get("node.description")
                .unwrap_or_default()
                .to_owned(),
            nick: properties.get("node.nick").unwrap_or_default().to_owned(),
            media_class: media_class.to_owned(),
        })
    }
}

pub fn list_devices(timeout: Duration) -> Result<Vec<AudioDevice>> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopBox::new(None)?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None)?;
    let core = context.connect(None)?;
    let registry = core.get_registry()?;

    let found: Rc<RefCell<Vec<AudioDevice>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&found);

    let _listener = registry
        .add_listener_local()
        .global(move |global| {
            if !matches!(global.type_, ObjectType::Node) {
                return;
            }
            let Some(properties) = global.props else {
                return;
            };
            if let Some(device) = AudioDevice::from_properties(global.id, properties) {
                sink.borrow_mut().push(device);
            }
        })
        .register();

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        mainloop
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(20)));
    }

    let mut devices = found.borrow().clone();
    devices.sort_by(|left, right| {
        left.display_name()
            .to_lowercase()
            .cmp(&right.display_name().to_lowercase())
    });
    devices.dedup_by_key(|device| device.id);
    Ok(devices)
}

pub fn list_sinks(timeout: Duration) -> Result<Vec<AudioDevice>> {
    Ok(list_devices(timeout)?
        .into_iter()
        .filter(|device| device.is_sink() && !device.is_virtual_sink())
        .collect())
}

#[cfg(test)]
mod tests;
