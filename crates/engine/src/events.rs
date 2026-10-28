use std::sync::Arc;

use pipewire_effects_core::AudioFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamRole {
    Capture,
    Playback,
}

impl StreamRole {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    Unconnected,
    Connecting,
    Paused,
    Streaming,
    Error,
}

impl ConnectionState {
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

    #[must_use]
    pub const fn is_streaming(self) -> bool {
        matches!(self, Self::Streaming)
    }

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

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EngineEvent {
    Started {
        capture_node: String,
        format: AudioFormat,
    },
    FormatNegotiated {
        role: StreamRole,
        format: AudioFormat,
    },
    ConnectionChanged {
        role: StreamRole,
        state: ConnectionState,
    },
    Failed {
        message: String,
    },
    Stopped,
}

pub type EventSink = Arc<dyn Fn(EngineEvent) + Send + Sync>;

#[must_use]
pub fn ignore_events() -> EventSink {
    Arc::new(|_| {})
}

#[cfg(test)]
mod tests;
