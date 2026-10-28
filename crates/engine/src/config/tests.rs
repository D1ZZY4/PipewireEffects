use super::*;

#[test]
fn the_default_configuration_is_valid() {
    let config = EngineConfig::default();
    assert!(config.validate().is_ok());
    assert_eq!(config.format, AudioFormat::new(48_000, 2));
}

#[test]
fn the_target_fill_is_derived_from_the_latency_setting() {
    let config = EngineConfig {
        target_latency: Duration::from_millis(20),
        ..EngineConfig::default()
    };
    assert_eq!(config.target_fill_samples(), 1_920);
}

#[test]
fn the_ring_is_always_much_larger_than_the_target() {
    let config = EngineConfig {
        target_latency: Duration::from_millis(500),
        ring_capacity: Duration::from_millis(500),
        ..EngineConfig::default()
    };
    assert!(config.ring_capacity_samples() >= config.target_fill_samples() * 4);
}

#[test]
fn a_ring_smaller_than_the_target_is_rejected() {
    let config = EngineConfig {
        target_latency: Duration::from_millis(200),
        ring_capacity: Duration::from_millis(50),
        ..EngineConfig::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn an_implausible_format_is_rejected() {
    let config = EngineConfig {
        format: AudioFormat::new(0, 0),
        ..EngineConfig::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn an_empty_node_name_is_rejected() {
    let config = EngineConfig {
        capture_node_name: String::new(),
        ..EngineConfig::default()
    };
    assert!(config.validate().is_err());
}
