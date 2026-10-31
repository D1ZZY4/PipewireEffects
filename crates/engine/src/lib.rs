mod capture;
pub mod command;
pub mod config;
mod connection;
mod defaults;
pub mod device;
mod engine;
mod listener;
mod meters;
mod processing;
mod processors;
mod reference;
mod runtime;
mod shared;
mod spectrum;
mod streams;
mod target;

pub use command::{EffectSlot, EngineCommand};
pub use config::{AUTO_TARGET, EngineConfig, MAX_EFFECTS, OUTPUT_NODE_NAME, virtual_node_name};
pub use device::{DeviceInfo, list_devices};
pub use engine::EngineHandle;
pub use error::{EngineError, EngineResult};
pub use meters::MeterSnapshot;
pub(crate) use processing::cpu;
pub use processing::gpu;
pub use reference::ReferenceBus;
pub use shared::{EngineState, Shared, Snapshot, StatsSnapshot};
pub use spectrum::{BANDS, FLOOR_DB, Spectrum, band_centre};

mod error;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
