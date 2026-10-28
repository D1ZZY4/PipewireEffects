//! # pipewire-effects-dsp
//!
//! Pure-Rust audio effects and the DSP primitives they are built from.
//!
//! This crate is deliberately the only place that contains signal processing,
//! and it has exactly one dependency — `pipewire-effects-core` — for the
//! [`AudioEffect`] trait and the parameter types. It knows nothing about
//! PipeWire, GTK, presets or threads, which is what makes every filter here
//! testable in isolation against its analytic frequency response.
//!
//! ## Layout
//!
//! | Module | Contents |
//! |---|---|
//! | [`biquad`] | Second-order IIR sections, designed with the RBJ cookbook |
//! | [`util`] | dB conversions, ramps and one-pole smoothing |
//! | [`effects`] | The effects themselves, one module each |
//!
//! ## Adding an effect
//!
//! 1. Create `src/effects/your_effect.rs`.
//! 2. Build a [`ParamSet`](pipewire_effects_core::ParamSet) in `new()`, and
//!    describe it with `ParamSpec`s.
//! 3. Implement [`AudioEffect`]. Read parameters once per block into locals;
//!    never allocate or lock in `process`.
//! 4. Export a `DEF` constant and add it to [`REGISTRY`].
//!
//! Nothing else needs to change: the interface builds its controls from the
//! parameter specs, and presets address parameters by identifier.
//!
//! [`AudioEffect`]: pipewire_effects_core::AudioEffect

pub mod biquad;
pub mod effects;
pub mod util;

use pipewire_effects_core::EffectDef;

/// Every effect this build provides, in the order the interface lists them.
///
/// This is the single place effects become visible to the rest of the
/// application. `pipewire-effects-ui` iterates it to build the "add effect"
/// browser, and the command-line renderer resolves effect names through it.
pub static REGISTRY: &[EffectDef] = &[effects::eq::DEF, effects::gain::DEF, effects::limiter::DEF];

/// Returns [`REGISTRY`] as a `'static` slice, ready for
/// [`ChainController::new`](pipewire_effects_core::ChainController::new).
#[must_use]
pub const fn registry() -> &'static [EffectDef] {
    REGISTRY
}
