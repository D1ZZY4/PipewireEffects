use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PresetError {
    #[error("failed to {action} `{path}`: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("`{path}` is not a valid preset: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "preset `{name}` uses schema version {found}, but this build understands up to {supported}"
    )]
    UnsupportedSchema {
        name: String,
        found: u32,
        supported: u32,
    },

    #[error("no preset named `{0}`")]
    NotFound(String),

    #[error("`{0}` is not a usable preset name")]
    InvalidName(String),

    #[error("could not determine a directory to store presets in")]
    NoDataDirectory,
}

pub type Result<T, E = PresetError> = std::result::Result<T, E>;
