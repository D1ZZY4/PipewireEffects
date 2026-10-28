//! The effects this build provides.
//!
//! Every effect is a self-contained module exporting a `DEF` constant that
//! plugs into the registry, plus the concrete type behind it. Nothing here
//! knows about PipeWire, GTK or presets; effects see [`AudioBlock`]s and
//! [`ParamSet`]s and nothing else.
//!
//! [`AudioBlock`]: pipewire_effects_core::AudioBlock
//! [`ParamSet`]: pipewire_effects_core::ParamSet

pub mod eq;
pub mod gain;
pub mod limiter;
