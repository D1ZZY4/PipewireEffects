use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    #[error("effect `{effect}` has no parameter named `{param}`")]
    UnknownParameter { effect: String, param: String },

    #[error("parameter `{param}` expects a value in {min}..={max}, got {value}")]
    ValueOutOfRange {
        param: String,
        value: f32,
        min: f32,
        max: f32,
    },

    #[error("parameter `{param}` expects a {expected} value, got {got}")]
    ValueTypeMismatch {
        param: String,
        expected: &'static str,
        got: &'static str,
    },

    #[error("chain index {index} is out of bounds for a chain of {len} effect(s)")]
    IndexOutOfRange { index: usize, len: usize },

    #[error("no effect registered under the name `{0}`")]
    UnknownEffectKind(String),

    #[error("unsupported audio format: {0}")]
    UnsupportedFormat(String),

    #[error("the realtime command queue is full; the audio thread is not consuming commands")]
    CommandQueueFull,

    #[error("the chain is full: it holds {capacity} effect(s), which is the maximum")]
    ChainFull { capacity: usize },
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
