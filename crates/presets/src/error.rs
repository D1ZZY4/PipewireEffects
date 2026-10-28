use pwe_core::CoreError;
use std::fmt;

#[derive(Debug)]
pub enum PresetError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidName(String),
    InvalidValue(String),
    NotFound(String),
    UnsupportedVersion { found: u32, supported: u32 },
    MissingSection(&'static str),
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "file system error: {error}"),
            Self::Json(error) => write!(f, "malformed preset: {error}"),
            Self::InvalidName(name) => write!(f, "'{name}' is not a valid preset name"),
            Self::InvalidValue(detail) => write!(f, "invalid parameter value: {detail}"),
            Self::NotFound(name) => write!(f, "no preset named '{name}'"),
            Self::UnsupportedVersion { found, supported } => {
                write!(
                    f,
                    "preset schema version {found} is newer than the supported version {supported}"
                )
            }
            Self::MissingSection(section) => write!(f, "the preset has no '{section}' section"),
        }
    }
}

impl std::error::Error for PresetError {}

impl From<std::io::Error> for PresetError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for PresetError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<CoreError> for PresetError {
    fn from(error: CoreError) -> Self {
        Self::InvalidValue(error.to_string())
    }
}

pub type PresetResult<T> = Result<T, PresetError>;
