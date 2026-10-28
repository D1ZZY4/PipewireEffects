//! # pipewire-effects-presets
//!
//! Preset storage: the file format, the presets compiled into the binary, and
//! the directory user presets are read from and written to.
//!
//! ## Why this is a separate crate
//!
//! A preset is a *compatibility surface*. Once presets are shared between
//! people, their format is as much a public interface as the command line, so
//! it gets its own crate with its own tests and its own schema version rather
//! than being an implementation detail of the interface.
//!
//! This crate deliberately does **not** depend on `pipewire-effects-dsp`.
//! Presets are data; resolving an effect kind or a parameter identifier to
//! something real needs a registry, and that registry is passed in by the
//! caller. That keeps the file format independent of which effects happen to
//! exist in a given build.
//!
//! ## Example
//!
//! ```no_run
//! use pipewire_effects_presets::PresetStore;
//!
//! let store = PresetStore::discover()?;
//! for info in store.list() {
//!     println!("{}{}", if info.builtin { "• " } else { "  " }, info.name);
//! }
//!
//! let preset = store.load("Bass Boost")?;
//! let recipe = preset.to_recipe();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod error;
pub mod model;
pub mod store;

pub use error::{PresetError, Result};
pub use model::{ChainSection, EffectEntry, Preset, SCHEMA_VERSION};
pub use store::{PresetInfo, PresetStore};
