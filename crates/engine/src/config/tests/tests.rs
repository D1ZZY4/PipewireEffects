use super::*;

#[test]
fn the_ring_holds_half_a_second_of_stereo_audio() {
    let config = EngineConfig::default();
    assert_eq!(config.ring_capacity_samples(), 192_000 * 2 / 2);
}

#[test]
fn each_pipeline_has_its_own_nodes_and_metadata_key() {
    let output = EngineConfig::for_kind(PipelineKind::Output);
    let input = EngineConfig::for_kind(PipelineKind::Input);
    assert_eq!(output.node_name, "pipewire-effects-sink");
    assert_eq!(input.node_name, "pipewire-effects-source");
    assert_ne!(output.own_nodes(), input.own_nodes());
    assert_eq!(metadata_key(PipelineKind::Input), "default.audio.source");
    assert_eq!(virtual_media_class(PipelineKind::Input), "Audio/Source");
}

#[test]
fn a_fresh_configuration_follows_the_system_default() {
    assert!(EngineConfig::default().follows_the_system_default());
}
