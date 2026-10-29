use std::fmt;

use crate::i18n::tr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    UnknownParameter(String),
    InvalidValue { id: String, detail: String },
    BufferCapacity { needed: usize, available: usize },
    ChainFull { capacity: usize },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownParameter(id) => {
                write!(f, "{}", tr("unknown parameter '{0}'", &[id]))
            }
            Self::InvalidValue { id, detail } => {
                write!(f, "{}", tr("invalid value for '{0}': {1}", &[id, detail]))
            }
            Self::BufferCapacity { needed, available } => write!(
                f,
                "{}",
                tr(
                    "buffer needs {0} frames but can hold {1}",
                    &[&needed.to_string(), &available.to_string()],
                )
            ),
            Self::ChainFull { capacity } => write!(
                f,
                "{}",
                tr("effect chain is full ({0} slots)", &[&capacity.to_string()])
            ),
        }
    }
}

impl std::error::Error for CoreError {}

pub type CoreResult<T> = Result<T, CoreError>;
