//! # pipewire-effects-ui
//!
//! The GTK4 and libadwaita interface.
//!
//! ## What is in here
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`action`] | The message type widgets emit, and the dispatch function |
//! | [`runtime`] | Shared state and the engine's lifecycle |
//! | [`window`] | The window, its sidebar and the periodic tick |
//! | [`chain`] | The chain list and the effect browser |
//! | [`editor`] | The parameter page, built from parameter specifications |
//! | [`app`] | Application bootstrap |
//!
//! ## The one idea worth knowing
//!
//! The editor never knows what an effect *is*. It renders whatever
//! `ParamSpec`s the effect declares: a float with a range becomes a slider, a
//! boolean becomes a switch, an enumeration becomes a drop-down, and a naming
//! convention (`band1_gain`) turns a flat list of forty-one settings into tidy
//! groups. Adding an effect to `pipewire-effects-dsp` therefore makes it fully
//! controllable here with no changes to this crate.
//!
//! ## Threading
//!
//! Everything in this crate runs on the GTK main thread. The audio engine runs
//! on its own thread and communicates through atomics and a queue, and the
//! interface polls its event channel twenty times a second. There is no
//! `unsafe` and no lock that the audio thread ever touches.

pub mod action;
pub mod app;
pub mod chain;
pub mod editor;
pub mod error;
pub mod runtime;
pub mod window;

pub use action::{Dispatch, UiAction};
pub use app::{APPLICATION_ID, run};
pub use error::{Result, UiError};
pub use runtime::Runtime;
