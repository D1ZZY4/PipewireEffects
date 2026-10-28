use thiserror::Error;

use pipewire_effects_core::CoreError;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EngineError {
    #[error("PipeWire reported an error: {0}")]
    PipeWire(#[from] pipewire::Error),

    #[error("could not start the audio thread: {0}")]
    ThreadSpawn(#[source] std::io::Error),

    #[error("unsupported engine configuration: {0}")]
    InvalidConfig(String),

    #[error("core error: {0}")]
    Core(#[from] CoreError),

    #[error("could not enumerate audio devices: {0}")]
    DeviceEnumeration(String),
}

pub type Result<T, E = EngineError> = std::result::Result<T, E>;
