pub mod biquad;
pub mod effects;
pub mod util;

use pipewire_effects_core::EffectDef;

pub static REGISTRY: &[EffectDef] = &[effects::eq::DEF, effects::gain::DEF, effects::limiter::DEF];

#[must_use]
pub const fn registry() -> &'static [EffectDef] {
    REGISTRY
}
