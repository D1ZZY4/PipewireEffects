//! # pipewire-effects-engine
//!
//! The audio engine: a PipeWire virtual sink, the DSP chain, and the lock-free
//! plumbing that connects them.
//!
//! ## The shape of the thing
//!
//! ```text
//!   application ──► virtual sink ──► ring buffer ──► effect chain ──► device
//!                        │                               │
//!                   input meters                   output meters
//!                        └─────────── stats ─────────┘
//! ```
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`config`] | What the engine is told to do |
//! | [`events`] | What the engine tells the interface |
//! | [`pipewire`] | The backend: two streams and a main loop |
//! | [`processor`] | The realtime path: ring buffer, chain, meters |
//! | [`stats`] | Counters that make glitches visible |
//! | [`devices`] | Enumerating playable devices |
//!
//! ## The three rules
//!
//! 1. **The audio thread never allocates, locks or frees.** Parameter changes
//!    travel through atomics; structural edits travel through a lock-free
//!    queue; retired effects are handed to a recycling queue that the engine's
//!    own non-realtime thread drains.
//! 2. **The interface never touches the chain.** It posts
//!    [`ChainCommand`](pipewire_effects_core::ChainCommand)s and reads meters
//!    and counters. The only shared mutable state is a set of atomic cells.
//! 3. **Problems are counted, not hidden.** Underruns, overruns, latency
//!    trims and dropped effects all have counters in [`stats`]. A loopback can
//!    appear to work while quietly dropping audio; the counters are how you
//!    find out.
//!
//! ## Using it
//!
//! The engine is told which effects exist rather than discovering them, which
//! is why this example supplies its own (empty) registry. In the real
//! application the registry comes from `pipewire-effects-dsp`.
//!
//! ```no_run
//! use pipewire_effects_core::{ChainController, EffectDef};
//! use pipewire_effects_engine::{Engine, EngineConfig, ignore_events};
//!
//! static EFFECTS: &[EffectDef] = &[];
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let controller = ChainController::new(EFFECTS);
//! let (engine, handles) =
//!     Engine::spawn(EngineConfig::default(), controller.command_queue(), ignore_events())?;
//!
//! // The interface now drives `controller` and reads `handles`.
//! println!("buffered: {:.1} ms", handles.latency_ms());
//! # drop(engine);
//! # Ok(())
//! # }
//! ```

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
