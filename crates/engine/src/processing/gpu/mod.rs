mod accelerator;
mod dispatch;
mod probe;
mod resources;
mod scaler;

use std::sync::Arc;

pub use accelerator::Accelerator;
pub use dispatch::scale;
pub use probe::{DeviceInfo, devices};
pub use scaler::Scaler;

use crate::shared::Shared;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioBackend {
    #[default]
    Cpu,
    Gpu,
    Both,
}

impl AudioBackend {
    pub fn parse(value: &str) -> Self {
        match value {
            "gpu" => Self::Gpu,
            "both" => Self::Both,
            _ => Self::Cpu,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Both => "both",
        }
    }

    pub fn accelerator(
        self,
        capacity: usize,
        shared: Arc<Shared>,
    ) -> Option<Box<dyn pwe_core::Accelerator>> {
        match self {
            Self::Cpu => None,
            Self::Gpu => Accelerator::new(capacity, false, shared)
                .map(|value| Box::new(value) as Box<dyn pwe_core::Accelerator>),
            Self::Both => Accelerator::new(capacity, true, shared)
                .map(|value| Box::new(value) as Box<dyn pwe_core::Accelerator>),
        }
    }
}

pub fn available() -> bool {
    !devices().is_empty()
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
