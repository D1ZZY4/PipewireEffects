use std::fmt;

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
            Self::PipeWire(error) => write!(f, "PipeWire error: {error}"),
            Self::StreamUnavailable(which) => write!(f, "the {which} stream is not available"),
            Self::NoDevice(target) => write!(f, "no audio device matches '{target}'"),
            Self::InvalidName(name) => write!(f, "'{name}' is not a usable PipeWire node name"),
            Self::AlreadyRunning => write!(f, "the engine is already running"),
            Self::NotConnected => write!(f, "the engine is not connected to PipeWire"),
            Self::ThreadSpawn(detail) => {
                write!(f, "the engine thread could not be started: {detail}")
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
