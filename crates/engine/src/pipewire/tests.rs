use super::props::{capture_properties, format_pod, playback_properties};
use super::samples::{is_directly_viewable, read_samples, write_samples};
use super::*;
use crate::config::NODE_GROUP;

#[test]
fn format_pods_serialise_for_every_supported_channel_count() {
    for channels in [1_u16, 2, 6] {
        let format = AudioFormat::new(48_000, channels);
        let bytes = format_pod(format);
        assert!(!bytes.is_empty(), "no pod produced for {channels} channels");
        let pod = Pod::from_bytes(&bytes).expect("pod should parse");
        assert_eq!(negotiated_format(pod), Some(format));
    }
}

#[test]
fn a_round_trip_through_the_byte_buffer_preserves_samples() {
    let samples = [0.0_f32, 0.25, -0.5, 1.0, -1.0, 3.5];
    let mut bytes = vec![0_u8; samples.len() * 4];
    write_samples(&samples, &mut bytes);

    let mut scratch = Vec::new();
    let read = read_samples(&bytes, &mut scratch, 2);
    assert_eq!(read, &samples);
}

#[test]
fn a_misaligned_buffer_still_round_trips() {
    let samples = [0.125_f32, -0.25, 0.5, -1.0];
    let mut backing = vec![0_u8; samples.len() * 4 + 1];
    let bytes = &mut backing[1..];
    write_samples(&samples, bytes);

    let mut scratch = Vec::new();
    let read = read_samples(bytes, &mut scratch, 2);
    assert_eq!(read, &samples);
}

#[test]
fn reading_ignores_a_trailing_partial_frame() {
    let bytes = vec![0_u8; 4 * 5]; // five samples, not a whole stereo frame
    let mut scratch = Vec::new();
    let read = read_samples(&bytes, &mut scratch, 2);
    assert_eq!(read.len(), 4, "a partial frame must not be exposed");
}

#[test]
fn reading_a_buffer_of_whole_frames_keeps_all_of_them() {
    let bytes = vec![0_u8; 4 * 6];
    let mut scratch = Vec::new();
    assert_eq!(read_samples(&bytes, &mut scratch, 2).len(), 6);
}

#[test]
fn scratch_reuse_does_not_leak_between_reads() {
    let mut scratch = Vec::new();
    let loud = vec![0xFF_u8; 4 * 4];
    assert_eq!(read_samples(&loud, &mut scratch, 2).len(), 4);

    let quiet = vec![0_u8; 4 * 2];
    let read = read_samples(&quiet, &mut scratch, 2);
    assert_eq!(read.len(), 2);
    assert!(read.iter().all(|sample| *sample == 0.0));
}

#[test]
fn direct_viewing_is_allowed_only_when_alignment_permits() {
    let aligned = [0_f32; 4];
    assert_eq!(
        is_directly_viewable(aligned.as_ptr().cast::<u8>()),
        cfg!(target_endian = "little")
    );

    let backing = [0_u8; 8];
    #[allow(clippy::cast_ptr_alignment)]
    let misaligned = unsafe { backing.as_ptr().add(1) };
    assert!(!is_directly_viewable(misaligned));
}

#[test]
fn write_samples_never_writes_past_the_destination() {
    let samples = [1.0_f32; 8];
    let mut bytes = vec![0_u8; 4 * 4];
    write_samples(&samples, &mut bytes);
    let mut scratch = Vec::new();
    assert_eq!(read_samples(&bytes, &mut scratch, 1).len(), 4);
}

#[test]
fn connection_states_map_one_to_one() {
    use pw::stream::StreamState;
    assert_eq!(
        connection_state(StreamState::Streaming),
        ConnectionState::Streaming
    );
    assert_eq!(
        connection_state(StreamState::Paused),
        ConnectionState::Paused
    );
    assert_eq!(
        connection_state(StreamState::Connecting),
        ConnectionState::Connecting
    );
    assert_eq!(
        connection_state(StreamState::Unconnected),
        ConnectionState::Unconnected
    );
    assert_eq!(
        connection_state(StreamState::Error(String::new())),
        ConnectionState::Error
    );
}

#[test]
fn capture_properties_describe_a_virtual_sink() {
    let config = EngineConfig::default();
    let props = capture_properties(&config);
    assert_eq!(props.get("media.class"), Some("Audio/Sink"));
    assert_eq!(props.get("node.virtual"), Some("true"));
    assert_eq!(
        props.get("node.name"),
        Some(config.capture_node_name.as_str())
    );
    assert_eq!(props.get("node.link-group"), Some(NODE_GROUP));
}

#[test]
fn playback_properties_target_the_configured_device() {
    let config = EngineConfig {
        target_sink: Some(String::from("alsa_output.test")),
        ..EngineConfig::default()
    };
    let props = playback_properties(&config);
    assert_eq!(props.get("target.object"), Some("alsa_output.test"));
    assert_eq!(props.get("media.category"), Some("Playback"));
}

#[test]
fn playback_properties_omit_the_target_when_none_is_set() {
    let props = playback_properties(&EngineConfig::default());
    assert_eq!(props.get("target.object"), None);
}
