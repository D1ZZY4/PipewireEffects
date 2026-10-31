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
