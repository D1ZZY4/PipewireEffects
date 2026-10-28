pub mod config;
pub mod devices;
pub mod error;
pub mod events;
pub mod pipewire;
pub mod processor;
pub mod stats;

pub use config::{EngineConfig, NODE_GROUP};
pub use devices::{AudioDevice, DEFAULT_DISCOVERY_TIMEOUT, list_devices, list_sinks};
pub use error::{EngineError, Result};
pub use events::{ConnectionState, EngineEvent, EventSink, StreamRole, ignore_events};
pub use pipewire::{Engine, EngineHandles};
pub use processor::{CaptureSource, ProcessorReport, RtProcessor, recycle_queue};
pub use stats::{StatsSnapshot, StreamStats};
