use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use spa::param::ParamType;
use spa::param::audio::AudioInfoRaw;

use pipewire_effects_core::AudioFormat;

use crate::config::{EngineConfig, NODE_GROUP};

pub(super) fn capture_properties(config: &EngineConfig) -> pw::properties::PropertiesBox {
    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CLASS => "Audio/Sink",
        *pw::keys::MEDIA_ROLE => "DSP",
        *pw::keys::NODE_NAME => config.capture_node_name.clone(),
        *pw::keys::NODE_DESCRIPTION => config.capture_node_description.clone(),
        *pw::keys::NODE_VIRTUAL => "true",
        *pw::keys::NODE_GROUP => NODE_GROUP,
        *pw::keys::AUDIO_CHANNELS => config.format.channels.to_string(),
    };
    props.insert("node.link-group", NODE_GROUP);
    props
}

pub(super) fn playback_properties(config: &EngineConfig) -> pw::properties::PropertiesBox {
    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Playback",
        *pw::keys::MEDIA_ROLE => "DSP",
        *pw::keys::NODE_NAME => config.playback_node_name.clone(),
        *pw::keys::NODE_GROUP => NODE_GROUP,
        *pw::keys::AUDIO_CHANNELS => config.format.channels.to_string(),
    };
    if let Some(target) = &config.target_sink {
        props.insert(*pw::keys::TARGET_OBJECT, target.clone());
    }
    props.insert("node.link-group", NODE_GROUP);
    props
}

pub(super) fn format_pod(format: AudioFormat) -> Vec<u8> {
    let mut info = AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::F32LE);
    info.set_rate(format.sample_rate);
    info.set_channels(u32::from(format.channels));

    if format.channels == 2 {
        let mut positions = [0_u32; spa::sys::SPA_AUDIO_MAX_CHANNELS as usize];
        positions[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
        positions[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
        info.set_position(positions);
    }

    let object = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };

    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .map_or_else(|_| Vec::new(), |(cursor, _)| cursor.into_inner())
}
