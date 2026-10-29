use pwe_core::CoreError;
use pwe_core::i18n::tr;
use std::fmt;

#[derive(Debug)]
pub enum PresetError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidName(String),
    InvalidValue(String),
    NotFound(String),
    UnsupportedVersion { found: u32, supported: u32 },
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{}", tr("file system error: {0}", &[&error.to_string()])),
            Self::Json(error) => {
                write!(f, "{}", tr("malformed preset: {0}", &[&error.to_string()]))
            }
            Self::InvalidName(name) => {
                write!(f, "{}", tr("'{0}' is not a valid preset name", &[name]))
            }
            Self::InvalidValue(detail) => {
                write!(f, "{}", tr("invalid parameter value: {0}", &[detail]))
            }
            Self::NotFound(name) => write!(f, "{}", tr("no preset named '{0}'", &[name])),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "{}",
                tr(
                    "preset schema version {0} is newer than the supported version {1}",
                    &[&found.to_string(), &supported.to_string()],
                )
            ),
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
