use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    UnknownParameter(String),
    InvalidValue { id: String, detail: String },
    BufferCapacity { needed: usize, available: usize },
    ChainFull { capacity: usize },
    InvalidIndex { index: usize, len: usize },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownParameter(id) => write!(f, "unknown parameter '{id}'"),
            Self::InvalidValue { id, detail } => write!(f, "invalid value for '{id}': {detail}"),
            Self::BufferCapacity { needed, available } => {
                write!(f, "buffer needs {needed} frames but can hold {available}")
            }
            Self::ChainFull { capacity } => write!(f, "effect chain is full ({capacity} slots)"),
            Self::InvalidIndex { index, len } => {
                write!(
                    f,
                    "index {index} is out of range for a chain of {len} effects"
                )
            }
        }
    }
}

impl std::error::Error for CoreError {}

pub type CoreResult<T> = Result<T, CoreError>;
