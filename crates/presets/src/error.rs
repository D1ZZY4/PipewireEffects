//! Errors from loading, saving and listing presets.

use std::path::PathBuf;

use thiserror::Error;

/// Anything that can go wrong while dealing with preset files.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PresetError {
    /// The file could not be read or written.
    #[error("failed to {action} `{path}`: {source}")]
    Io {
        /// What we were trying to do, for example "read" or "write".
        action: &'static str,
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The file is not valid JSON, or does not match the schema.
    #[error("`{path}` is not a valid preset: {source}")]
    Parse {
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: serde_json::Error,
    },

    /// The preset claims a schema version this build does not understand.
    #[error(
        "preset `{name}` uses schema version {found}, but this build understands up to {supported}"
    )]
    UnsupportedSchema {
        /// The preset's name.
        name: String,
        /// The version found in the file.
        found: u32,
        /// The newest version this build can read.
        supported: u32,
    },

    /// A preset name was requested that does not exist.
    #[error("no preset named `{0}`")]
    NotFound(String),

    /// A preset name contained characters that cannot be used in a file name.
    #[error("`{0}` is not a usable preset name")]
    InvalidName(String),

    /// The user's data directory could not be determined.
    #[error("could not determine a directory to store presets in")]
    NoDataDirectory,
}

/// Convenience alias for results produced by this crate.
pub type Result<T, E = PresetError> = std::result::Result<T, E>;
