//! Error types shared by every layer of PipewireEffects.
//!
//! The domain deliberately keeps a single, small error enum. Backends (PipeWire,
//! offline rendering, GTK) wrap it in their own errors rather than growing this
//! one, which keeps the domain vocabulary stable.

use thiserror::Error;

/// Errors produced by domain operations: parameter validation, chain editing
/// and format negotiation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// A parameter identifier was not recognised by the effect it was applied to.
    #[error("effect `{effect}` has no parameter named `{param}`")]
    UnknownParameter {
        /// The effect kind the lookup was performed on.
        effect: String,
        /// The parameter identifier that could not be resolved.
        param: String,
    },

    /// A numeric value fell outside the range declared by its [`ParamKind`].
    ///
    /// [`ParamKind`]: crate::param::ParamKind
    #[error("parameter `{param}` expects a value in {min}..={max}, got {value}")]
    ValueOutOfRange {
        /// The offending parameter.
        param: String,
        /// The rejected value.
        value: f32,
        /// Inclusive lower bound from the parameter specification.
        min: f32,
        /// Inclusive upper bound from the parameter specification.
        max: f32,
    },

    /// A value of the wrong variant was assigned to a parameter.
    #[error("parameter `{param}` expects a {expected} value, got {got}")]
    ValueTypeMismatch {
        /// The offending parameter.
        param: String,
        /// The variant the specification declares.
        expected: &'static str,
        /// The variant that was supplied.
        got: &'static str,
    },

    /// An index-based operation referenced a slot that does not exist.
    #[error("chain index {index} is out of bounds for a chain of {len} effect(s)")]
    IndexOutOfRange {
        /// The requested index.
        index: usize,
        /// The current number of entries in the chain.
        len: usize,
    },

    /// An effect kind string did not match any entry in the registry.
    #[error("no effect registered under the name `{0}`")]
    UnknownEffectKind(String),

    /// The negotiated audio format is not something we can process.
    #[error("unsupported audio format: {0}")]
    UnsupportedFormat(String),

    /// The realtime command queue could not accept a structural edit.
    ///
    /// This means the audio thread is not draining commands, which indicates a
    /// stalled or dead backend rather than a user error.
    #[error("the realtime command queue is full; the audio thread is not consuming commands")]
    CommandQueueFull,
}

/// Convenience alias for results produced by this crate.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;
