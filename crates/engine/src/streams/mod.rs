pub(crate) mod stream_setup;
mod stream_spec;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use pipewire as pw;
use pw::spa;
use pw::spa::param::format::{MediaSubtype, MediaType};
use pw::spa::param::format_utils;
use pw::spa::pod::serialize::PodSerializer;
use pw::spa::pod::{Object, Pod, Value};
use std::io::Cursor;

pub fn audio_format_pod(channels: usize) -> Option<Vec<u8>> {
    let mut info = spa::param::audio::AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::F32LE);
    info.set_channels(channels.max(1) as u32);
    let object = Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    let (cursor, _) =
        PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(object)).ok()?;
    Some(cursor.into_inner())
}

pub fn parse_rate(param: &Pod) -> Option<u32> {
    let (media_type, media_subtype) = format_utils::parse_format(param).ok()?;
    if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
        return None;
    }
    let mut info = spa::param::audio::AudioInfoRaw::new();
    info.parse(param).ok()?;
    let rate = info.rate();
    (rate > 0).then_some(rate)
}

pub fn bytes_to_samples(bytes: &[u8], destination: &mut [f32]) -> usize {
    let count = (bytes.len() / 4).min(destination.len());
    for (index, sample) in destination.iter_mut().enumerate().take(count) {
        let start = index * 4;
        *sample = f32::from_le_bytes([
            bytes[start],
            bytes[start + 1],
            bytes[start + 2],
            bytes[start + 3],
        ]);
    }
    count
}

pub fn samples_to_bytes(samples: &[f32], destination: &mut [u8]) -> usize {
    let count = (destination.len() / 4).min(samples.len());
    for (index, sample) in samples.iter().enumerate().take(count) {
        let start = index * 4;
        destination[start..start + 4].copy_from_slice(&sample.to_le_bytes());
    }
    count
}

pub fn stream_properties(
    media_class: &str,
    category: &str,
    role: &str,
    node_name: &str,
    node_description: &str,
    group: &str,
) -> pw::properties::PropertiesBox {
    let mut properties = pw::properties::PropertiesBox::new();
    properties.insert("media.type", "Audio");
    properties.insert("media.category", category);
    properties.insert("media.role", role);
    properties.insert("media.class", media_class);
    properties.insert("node.name", node_name);
    properties.insert("node.description", node_description);
    properties.insert("node.group", group);
    properties.insert("node.link-group", group);
    properties.insert("audio.position", "FL,FR");
    disable_session_restore(&mut properties);
    properties
}

pub fn disable_session_restore(properties: &mut pw::properties::PropertiesBox) {
    properties.insert("state.restore-props", "false");
    properties.insert("state.restore-target", "false");
}
