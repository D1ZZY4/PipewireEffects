mod analysis;
pub mod effects;
mod primitives;

pub use analysis::{BANDS, Loudness, SpectrumAnalyzer, band_centre};
pub use effects::registry;
pub use primitives::{Biquad, Coefficients, Fft, FilterKind, Knob};
pub use registry::{EFFECTS, EffectCategory, EffectDescriptor, find, instantiate, rebuild};

pub use analysis::spectrum;

pub(crate) use analysis::r128;
pub(crate) use primitives::{biquad, fft, knob};

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
