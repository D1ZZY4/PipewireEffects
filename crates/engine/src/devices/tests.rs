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

    let device = AudioDevice::from_properties(7, properties.as_ref()).expect("should be a device");
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
    assert_eq!(device.display_name(), "some.sink");
}
