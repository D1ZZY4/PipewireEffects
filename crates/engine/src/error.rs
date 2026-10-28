//! Errors produced by the audio engine.

use thiserror::Error;

use pipewire_effects_core::CoreError;

/// Anything that can go wrong starting or running the engine.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EngineError {
    /// The PipeWire client library refused an operation.
    #[error("PipeWire reported an error: {0}")]
    PipeWire(#[from] pipewire::Error),

    /// The dedicated audio thread could not be created.
    #[error("could not start the audio thread: {0}")]
    ThreadSpawn(#[source] std::io::Error),

    /// The engine was asked to run with a format it cannot process.
    #[error("unsupported engine configuration: {0}")]
    InvalidConfig(String),

    /// A domain operation failed while setting the engine up.
    #[error("core error: {0}")]
    Core(#[from] CoreError),

    /// Devices could not be enumerated.
    #[error("could not enumerate audio devices: {0}")]
    DeviceEnumeration(String),
}

/// Convenience alias for results produced by this crate.
pub type Result<T, E = EngineError> = std::result::Result<T, E>;
