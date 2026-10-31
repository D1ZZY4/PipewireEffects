use super::*;

#[test]
fn little_endian_round_trip() {
    let samples = [0.0f32, 0.5, -0.25, 1.0];
    let mut bytes = [0u8; 16];
    assert_eq!(samples_to_bytes(&samples, &mut bytes), 4);
    assert_eq!(bytes[4..8], 0.5f32.to_le_bytes());
    let mut restored = [0.0f32; 4];
    assert_eq!(bytes_to_samples(&bytes, &mut restored), 4);
    for (restored, original) in restored.iter().zip(samples.iter()) {
        assert!((restored - original).abs() < 1.0e-6);
    }
}

#[test]
fn short_destinations_are_respected() {
    let samples = [1.0f32, 2.0, 3.0];
    let mut bytes = [0u8; 4];
    assert_eq!(samples_to_bytes(&samples, &mut bytes), 1);
    let mut restored = [0.0f32; 2];
    assert_eq!(bytes_to_samples(&bytes, &mut restored), 1);
}

#[test]
fn a_format_pod_carries_the_channel_count() {
    let pod = audio_format_pod(2).expect("pod should serialize");
    assert!(Pod::from_bytes(&pod).is_some());
}

#[test]
fn internal_streams_opt_out_of_the_session_manager_restore() {
    let properties = stream_properties(
        "Audio/Sink",
        "Capture",
        "DSP",
        "pipewire-effects-sink",
        "Sink",
        "pipewire-effects",
    );
    let properties: &pw::properties::Properties = properties.as_ref();
    assert_eq!(properties.get("state.restore-props"), Some("false"));
    assert_eq!(properties.get("state.restore-target"), Some("false"));
    assert_eq!(properties.get("node.name"), Some("pipewire-effects-sink"));
}
