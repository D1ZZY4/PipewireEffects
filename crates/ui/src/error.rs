//! Errors the interface can hit before it has anything to show.

use thiserror::Error;

/// Problems that stop the interface from doing its job.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UiError {
    /// The engine could not be started.
    #[error("could not start the audio engine: {0}")]
    Engine(#[from] pipewire_effects_engine::EngineError),

    /// A preset could not be read or written.
    #[error("preset error: {0}")]
    Preset(#[from] pipewire_effects_presets::PresetError),

    /// A chain edit was rejected.
    #[error("chain error: {0}")]
    Chain(#[from] pipewire_effects_core::CoreError),

    /// The runtime's event channel was already closed.
    #[error("the engine event channel is closed")]
    EngineChannelClosed,
}

/// Convenience alias for results produced by this crate.
pub type Result<T, E = UiError> = std::result::Result<T, E>;
