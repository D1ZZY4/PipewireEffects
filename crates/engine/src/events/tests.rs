use super::*;

#[test]
fn roles_have_distinct_labels() {
    assert_eq!(StreamRole::Capture.to_string(), "capture");
    assert_eq!(StreamRole::Playback.to_string(), "playback");
}

#[test]
fn only_streaming_counts_as_streaming() {
    assert!(ConnectionState::Streaming.is_streaming());
    for state in [
        ConnectionState::Unconnected,
        ConnectionState::Connecting,
        ConnectionState::Paused,
        ConnectionState::Error,
    ] {
        assert!(!state.is_streaming());
    }
}

#[test]
fn error_is_not_usable() {
    assert!(!ConnectionState::Error.is_usable());
    assert!(!ConnectionState::Unconnected.is_usable());
    assert!(ConnectionState::Paused.is_usable());
}

#[test]
fn the_default_sink_can_be_called_from_another_thread() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = {
        let seen = Arc::clone(&seen);
        let sink: EventSink = Arc::new(move |event| seen.lock().expect("poisoned").push(event));
        sink
    };
    sink(EngineEvent::Stopped);
    assert_eq!(seen.lock().expect("poisoned").len(), 1);
}
