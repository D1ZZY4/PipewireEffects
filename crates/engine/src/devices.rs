//! Enumerating the audio devices the engine can play on.
//!
//! This is a *separate, short-lived* PipeWire client rather than a facility of
//! the running engine. Enumerating devices means running a main loop until the
//! server has told us about everything it has, which takes tens of
//! milliseconds; doing it on a throwaway connection keeps that cost away from
//! the audio thread, and means the device list can be refreshed without
//! disturbing playback.
//!
//! Node properties come straight out of the registry's global objects, so no
//! proxy objects are created and nothing needs to be torn down beyond dropping
//! the loop.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use pipewire as pw;
use pw::spa;
use pw::types::ObjectType;

use crate::error::Result;

/// How long to wait for the server to describe its objects.
///
/// The registry sends every global as soon as the client connects, so this is
/// generous; it exists only so that a wedged server cannot hang the interface.
pub const DEFAULT_DISCOVERY_TIMEOUT: Duration = Duration::from_millis(400);

/// A PipeWire node that audio can flow to or from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    /// The node's global id. Changes every session, so do not persist it.
    pub id: u32,
    /// `object.serial`, unique for the lifetime of the node.
    pub serial: u64,
    /// `node.name`. This is the stable identifier to store in settings.
    pub name: String,
    /// `node.description`, which is what a person should be shown.
    pub description: String,
    /// `node.nick`, an optional shorter name.
    pub nick: String,
    /// `media.class`, for example `Audio/Sink`.
    pub media_class: String,
}

impl AudioDevice {
    /// Whether audio can be played on this device.
    #[must_use]
    pub fn is_sink(&self) -> bool {
        self.media_class.starts_with("Audio/Sink")
    }

    /// Whether audio can be captured from this device.
    #[must_use]
    pub fn is_source(&self) -> bool {
        self.media_class.starts_with("Audio/Source")
    }

    /// Whether this is a monitor of a sink rather than real hardware.
    #[must_use]
    pub fn is_monitor(&self) -> bool {
        self.is_source() && self.name.ends_with(".monitor")
    }

    /// Whether this is a device provided by this application.
    #[must_use]
    pub fn is_virtual_sink(&self) -> bool {
        self.name.starts_with("pipewire-effects")
    }

    /// The name to show a person: the description when there is one.
    #[must_use]
    pub fn display_name(&self) -> &str {
        if self.description.is_empty() {
            &self.name
        } else {
            &self.description
        }
    }

    /// A second line for the interface, usually the stable node name.
    #[must_use]
    pub fn detail(&self) -> &str {
        if self.nick.is_empty() {
            &self.name
        } else {
            &self.nick
        }
    }

    /// Builds a device from a node's property dictionary.
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

/// Lists every audio device the server knows about.
///
/// # Errors
///
/// Returns an error if a PipeWire client cannot be created. A server that
/// simply has nothing to report yields an empty list.
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

/// Lists only the devices audio can be played on, excluding our own virtual
/// sink and monitor sources.
///
/// # Errors
///
/// As [`list_devices`].
pub fn list_sinks(timeout: Duration) -> Result<Vec<AudioDevice>> {
    Ok(list_devices(timeout)?
        .into_iter()
        .filter(|device| device.is_sink() && !device.is_virtual_sink())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw::properties::PropertiesBox;

    fn device(media_class: &str, name: &str) -> AudioDevice {
        AudioDevice {
            id: 1,
            serial: 1,
            name: name.to_owned(),
            description: String::new(),
            nick: String::new(),
            media_class: media_class.to_owned(),
        }
    }

    #[test]
    fn sinks_and_sources_are_recognised() {
        assert!(device("Audio/Sink", "alsa_output.x").is_sink());
        assert!(!device("Audio/Sink", "alsa_output.x").is_source());
        assert!(device("Audio/Source", "alsa_input.x").is_source());
    }

    #[test]
    fn virtual_sink_variants_are_recognised_as_sinks() {
        for media_class in ["Audio/Sink", "Audio/Sink/Virtual"] {
            let device = device(media_class, "pipewire-effects-sink");
            assert!(device.is_sink(), "{media_class} should be a sink");
            assert!(device.is_virtual_sink());
        }
    }

    #[test]
    fn monitor_sources_are_detected() {
        assert!(device("Audio/Source", "alsa_output.pci.monitor").is_monitor());
        assert!(!device("Audio/Source", "alsa_input.pci").is_monitor());
        assert!(!device("Audio/Sink", "alsa_output.pci.monitor").is_monitor());
    }

    #[test]
    fn the_display_name_falls_back_to_the_node_name() {
        let mut device = device("Audio/Sink", "alsa_output.pci");
        assert_eq!(device.display_name(), "alsa_output.pci");
        device.description = String::from("Built-in Audio");
        assert_eq!(device.display_name(), "Built-in Audio");
    }

    #[test]
    fn a_non_audio_node_is_not_a_device() {
        let properties = PropertiesBox::new();
        assert!(AudioDevice::from_properties(1, properties.as_ref()).is_none());
    }

    #[test]
    fn a_node_with_a_media_class_becomes_a_device() {
        let mut properties = PropertiesBox::new();
        properties.insert("media.class", "Audio/Sink");
        properties.insert("node.name", "alsa_output.test");
        properties.insert("node.description", "Test Output");
        properties.insert("node.nick", "TestOut");
        properties.insert("object.serial", "42");

        let device =
            AudioDevice::from_properties(7, properties.as_ref()).expect("should be a device");
        assert_eq!(device.id, 7);
        assert_eq!(device.serial, 42);
        assert_eq!(device.name, "alsa_output.test");
        assert_eq!(device.display_name(), "Test Output");
        assert_eq!(device.detail(), "TestOut");
        assert!(device.is_sink());
    }

    #[test]
    fn a_node_without_a_serial_still_builds() {
        let mut properties = PropertiesBox::new();
        properties.insert("media.class", "Audio/Sink");
        properties.insert("node.name", "some.sink");

        let device = AudioDevice::from_properties(3, properties.as_ref()).unwrap();
        assert_eq!(device.serial, 0);
        assert_eq!(device.description, "");
        // With no description the node name is what a person sees.
        assert_eq!(device.display_name(), "some.sink");
    }
}
