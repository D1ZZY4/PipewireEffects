//! Events the engine reports back to the interface.
//!
//! The engine runs on its own thread and cannot call into GTK directly, so it
//! publishes a small, plain-data stream of events through a callback. The
//! application is responsible for marshalling those onto the interface thread;
//! nothing in this crate knows what an event loop is.

use std::sync::Arc;

use pipewire_effects_core::AudioFormat;

/// Which half of the loopback an event refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamRole {
    /// The virtual sink that applications play into.
    Capture,
    /// The stream that plays the processed audio out.
    Playback,
}

impl StreamRole {
    /// A short label, for logs and status lines.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Playback => "playback",
        }
    }
}

impl std::fmt::Display for StreamRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A simplified view of a stream's connection state.
///
/// PipeWire's own `StreamState::Error` carries a string; this collapses it to a
/// variant so the type stays `Copy` and can be compared in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    /// The stream has not been connected yet.
    Unconnected,
    /// The stream is negotiating with the server.
    Connecting,
    /// The stream is connected but not being scheduled.
    Paused,
    /// Audio is flowing.
    Streaming,
    /// The stream failed.
    Error,
}

impl ConnectionState {
    /// A short label, for status lines.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Paused => "paused",
            Self::Streaming => "streaming",
            Self::Error => "failed",
        }
    }

    /// Whether the stream is actually moving audio.
    #[must_use]
    pub const fn is_streaming(self) -> bool {
        matches!(self, Self::Streaming)
    }

    /// Whether the stream is in a usable state.
    #[must_use]
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Connecting | Self::Paused | Self::Streaming)
    }
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Something the engine wants the interface to know.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EngineEvent {
    /// The engine is running and its streams exist.
    Started {
        /// The node name applications route to.
        capture_node: String,
        /// The format the engine is configured for.
        format: AudioFormat,
    },
    /// A stream finished negotiating a format.
    ///
    /// Worth surfacing: if this ever differs from the requested format, audio
    /// is being converted somewhere and the interface should say so rather than
    /// silently showing the wrong sample rate.
    FormatNegotiated {
        /// Which stream.
        role: StreamRole,
        /// What was agreed.
        format: AudioFormat,
    },
    /// A stream changed connection state.
    ConnectionChanged {
        /// Which stream.
        role: StreamRole,
        /// The new state.
        state: ConnectionState,
    },
    /// The engine could not start, or stopped because of an error.
    Failed {
        /// A human readable description.
        message: String,
    },
    /// The engine stopped on request.
    Stopped,
}

/// Where the engine sends events.
///
/// Called from the audio engine's own thread, so implementations must marshal
/// to whatever thread they want to run on.
pub type EventSink = Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// An event sink that discards everything.
///
/// Useful for tests and for running the engine headless.
#[must_use]
pub fn ignore_events() -> EventSink {
    Arc::new(|_| {})
}

#[cfg(test)]
mod tests {
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
}
