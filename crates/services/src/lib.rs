mod autostart;
mod error;
mod model;
mod store;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

pub use autostart::{APPLICATION_ID, DEFAULT_EXEC, autostart_path, set_enabled};
pub use error::{SettingsError, SettingsResult};
pub use model::{AUTO, PipelineSettings, Settings};
pub use store::{SettingsStore, config_home};
