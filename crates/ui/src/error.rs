use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UiError {
    #[error("could not start the audio engine: {0}")]
    Engine(#[from] pipewire_effects_engine::EngineError),

    #[error("preset error: {0}")]
    Preset(#[from] pipewire_effects_presets::PresetError),

    #[error("chain error: {0}")]
    Chain(#[from] pipewire_effects_core::CoreError),
}

pub type Result<T, E = UiError> = std::result::Result<T, E>;
