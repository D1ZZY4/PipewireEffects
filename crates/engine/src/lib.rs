mod capture;
pub mod command;
pub mod config;
mod connection;
mod defaults;
pub mod device;
mod engine;
mod processor;
mod processors;
mod runtime;
mod shared;
mod stream_setup;
mod streams;
mod target;

pub use command::{EffectSlot, EngineCommand};
pub use config::{AUTO_TARGET, EngineConfig, SINK_NODE_DESCRIPTION, SINK_NODE_NAME};
pub use device::{DeviceInfo, list_devices};
pub use engine::EngineHandle;
pub use error::{EngineError, EngineResult};
pub use shared::{EngineState, MeterSnapshot, Shared, Snapshot, StatsSnapshot};

mod error;
