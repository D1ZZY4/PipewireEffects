use super::*;

#[test]
fn the_output_pipeline_offers_a_sink_and_plays_to_the_chosen_device() {
    let config = EngineConfig::for_kind(PipelineKind::Output);
    let up = upstream(&config, "speakers");
    let down = downstream(&config, "speakers");
    assert_eq!(up.media_class, "Audio/Sink");
    assert_eq!(up.node_name, "pipewire-effects-sink");
    assert!(up.virtual_device);
    assert!(up.target.is_empty());
    assert_eq!(down.media_class, "Stream/Output/Audio");
    assert_eq!(down.target, "speakers");
    assert!(!down.virtual_device);
}

#[test]
fn the_input_pipeline_reads_the_chosen_device_and_offers_a_source() {
    let config = EngineConfig::for_kind(PipelineKind::Input);
    let up = upstream(&config, "mic");
    let down = downstream(&config, "mic");
    assert_eq!(up.media_class, "Stream/Input/Audio");
    assert_eq!(up.target, "mic");
    assert!(!up.virtual_device);
    assert_eq!(down.media_class, "Audio/Source");
    assert_eq!(down.node_name, "pipewire-effects-source");
    assert!(down.virtual_device);
    assert!(down.target.is_empty());
}

#[test]
fn only_the_stream_bound_to_a_device_asks_to_be_autoconnected() {
    let output = EngineConfig::for_kind(PipelineKind::Output);
    assert!(!upstream(&output, "speakers").autoconnects());
    assert!(downstream(&output, "speakers").autoconnects());

    let input = EngineConfig::for_kind(PipelineKind::Input);
    assert!(upstream(&input, "mic").autoconnects());
    assert!(!downstream(&input, "mic").autoconnects());
}

#[test]
fn a_virtual_node_never_carries_a_target() {
    for kind in pwe_core::ALL_PIPELINES {
        let config = EngineConfig::for_kind(kind);
        let spec = if kind == PipelineKind::Output {
            upstream(&config, "device")
        } else {
            downstream(&config, "device")
        };
        assert!(spec.virtual_device);
        assert!(spec.target.is_empty());
        assert!(!spec.autoconnects());
        let properties = spec.properties();
        assert_eq!(properties.get("node.dont-fallback"), Some("true"));
        assert_eq!(properties.get("priority.session"), Some("0"));
        assert_eq!(properties.get("node.virtual"), Some("true"));
    }
}
