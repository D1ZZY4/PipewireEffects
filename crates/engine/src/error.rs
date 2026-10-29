use std::fmt;

use pwe_core::i18n::{t, tr};

#[derive(Debug)]
pub enum EngineError {
    PipeWire(pipewire::Error),
    StreamUnavailable(&'static str),
    NoDevice(String),
    InvalidName(String),
    AlreadyRunning,
    NotConnected,
    ThreadSpawn(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PipeWire(error) => {
                write!(f, "{}", tr("PipeWire error: {0}", &[&error.to_string()]))
            }
            Self::StreamUnavailable(which) => {
                write!(f, "{}", tr("the {0} stream is not available", &[which]))
            }
            Self::NoDevice(target) => {
                write!(f, "{}", tr("no audio device matches '{0}'", &[target]))
            }
            Self::InvalidName(name) => {
                write!(
                    f,
                    "{}",
                    tr("'{0}' is not a usable PipeWire node name", &[name])
                )
            }
            Self::AlreadyRunning => write!(f, "{}", t("the engine is already running")),
            Self::NotConnected => write!(f, "{}", t("the engine is not connected to PipeWire")),
            Self::ThreadSpawn(detail) => {
                write!(
                    f,
                    "{}",
                    tr("the engine thread could not be started: {0}", &[detail])
                )
            }
        }
    }
}

impl std::error::Error for EngineError {}

impl From<pipewire::Error> for EngineError {
    fn from(error: pipewire::Error) -> Self {
        Self::PipeWire(error)
    }
}

pub type EngineResult<T> = Result<T, EngineError>;
