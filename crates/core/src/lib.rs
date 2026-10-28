//! # pipewire-effects-core
//!
//! The domain model of PipewireEffects, with no dependency on PipeWire, GTK or
//! any other backend. Everything here is plain data plus the interfaces that
//! the rest of the workspace implements or consumes.
//!
//! ## What lives here
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`audio`] | Interleaved `f32` buffers and the negotiated stream format |
//! | [`param`] | Parameter metadata, live atomic values and formatting |
//! | [`effect`] | The [`AudioEffect`] trait and the effect registry |
//! | [`chain`] | The ordered chain and the commands that mutate it |
//! | [`control`] | The interface-side model that drives the chain |
//! | [`rt`] | The lock-free ring buffer between capture and playback |
//! | [`meter`] | Level metering shared between audio and interface threads |
//!
//! ## Realtime rules
//!
//! Two rules run through the whole crate, and breaking either one will cause
//! audible dropouts:
//!
//! 1. **Nothing in the audio callback allocates or locks.** Parameter changes
//!    travel through [`AtomicParam`](param::AtomicParam) cells; structural
//!    edits travel through a lock-free queue of [`ChainCommand`]s; retired
//!    effects are handed out through a callback instead of being dropped.
//! 2. **The audio thread never frees memory.** See
//!    [`ChainCommand::apply`], which takes a `retire` closure precisely so
//!    that deallocation happens on the interface thread.
//!
//! The DSP crate is written to honour these rules, and the engine's realtime
//! path is the only place that calls into them at audio rate.

pub mod audio;
pub mod chain;
pub mod control;
pub mod effect;
pub mod error;
pub mod meter;
pub mod param;
pub mod rt;

pub use audio::{AudioBlock, AudioFormat, ChannelLayout, Sample};
pub use chain::{ChainCommand, ChainEntry, EffectChain};
pub use control::{
    ChainController, ChainModel, ChainRecipe, EffectModel, EffectRecipe, ParamRepr, RecipeReport,
};
pub use effect::{
    ApplyReport, AudioEffect, EffectCategory, EffectDef, EffectKind, apply_parameter_pairs,
    find_def, instantiate,
};
pub use error::{CoreError, Result};
pub use meter::{BlockMeter, MeterSnapshot, SharedMeters};
pub use param::{
    AtomicParam, Param, ParamId, ParamKind, ParamSet, ParamSpec, ParamUnit, ParamValue,
};
pub use rt::{AudioRing, TransferReport};
