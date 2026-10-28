//! The effect contract: what every processing block must implement, and how
//! effects are discovered.
//!
//! Two ideas carry the design:
//!
//! * An effect is *inert metadata plus a processing function*. Everything the
//!   interface needs is available through [`AudioEffect::parameters`], so the
//!   interface never has to know which effect it is looking at.
//! * Instances are produced by a registry of [`EffectDef`]s. The registry is
//!   plain data, which means the "add effect" browser, preset loading and the
//!   offline renderer can all be driven from it.

use std::fmt;

use crate::audio::{AudioBlock, AudioFormat};
use crate::error::{CoreError, Result};
use crate::param::{ParamId, ParamSet, ParamValue};

/// A stable, string-backed identifier for an effect type.
///
/// The string is what appears in preset files and on the command line, so it is
/// part of the public contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectKind(&'static str);

impl EffectKind {
    /// Creates a kind from a static string.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Borrows the underlying name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Groups effects in the "add effect" browser and in documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectCategory {
    /// Tone-shaping filters such as the parametric equaliser.
    Equalizer,
    /// Level-dependent processing such as limiters and compressors.
    Dynamics,
    /// Crossovers, band splits and other routing filters.
    Filter,
    /// Stereo imaging and crossfeed.
    Spatial,
    /// Level, metering and other utility blocks.
    Utility,
}

impl EffectCategory {
    /// Human readable name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Equalizer => "Equalizer",
            Self::Dynamics => "Dynamics",
            Self::Filter => "Filter",
            Self::Spatial => "Spatial",
            Self::Utility => "Utility",
        }
    }

    /// Every category, in the order the interface should list them.
    pub const ALL: [Self; 5] = [
        Self::Equalizer,
        Self::Dynamics,
        Self::Filter,
        Self::Spatial,
        Self::Utility,
    ];
}

impl fmt::Display for EffectCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A block of audio processing.
///
/// Implementations are owned by exactly one thread at a time. The audio thread
/// owns the instance while audio is running, so every method below must be
/// allocation-free and must not take locks.
pub trait AudioEffect: Send {
    /// The effect type this instance implements.
    fn kind(&self) -> EffectKind;

    /// The parameters this instance exposes, with their live values.
    ///
    /// The returned set is shared with the interface: writing to one of its
    /// [`AtomicParam`](crate::param::AtomicParam) cells is immediately visible
    /// on the audio thread.
    fn parameters(&self) -> &ParamSet;

    /// Called whenever the stream format changes, and once at startup.
    ///
    /// This is the only place an effect is allowed to allocate.
    fn prepare(&mut self, format: AudioFormat);

    /// Clears any internal state, for example filter memory and envelope
    /// followers. Called when audio restarts so that stale state cannot leak
    /// into a new stream.
    fn reset(&mut self);

    /// Processes one block in place.
    #[allow(unused_variables)]
    fn process(&mut self, block: &mut AudioBlock<'_>) {}

    /// The processing delay this effect introduces, in frames.
    ///
    /// The engine uses this to report total latency. Zero for anything without
    /// lookahead.
    fn latency_frames(&self) -> usize {
        0
    }

    /// A one-line, human readable description of the current settings.
    fn summary(&self) -> String {
        let mut parts = Vec::new();
        for (index, spec) in self.parameters().specs().iter().enumerate() {
            if let Some(value) = self.parameters().value(index) {
                parts.push(format!("{} {}", spec.label, spec.kind.format_value(value)));
            }
        }
        parts.join(", ")
    }
}

/// Static metadata describing an effect that can be instantiated.
#[derive(Clone, Copy)]
pub struct EffectDef {
    /// The effect type.
    pub kind: EffectKind,
    /// Display name.
    pub label: &'static str,
    /// One sentence explaining what the effect does.
    pub summary: &'static str,
    /// Where the effect appears in the browser.
    pub category: EffectCategory,
    /// Constructs a fresh instance with default parameters.
    pub factory: fn() -> Box<dyn AudioEffect>,
}

impl EffectDef {
    /// Builds a definition. Intended for use in `static` registry tables.
    #[must_use]
    pub const fn new(
        kind: EffectKind,
        label: &'static str,
        summary: &'static str,
        category: EffectCategory,
        factory: fn() -> Box<dyn AudioEffect>,
    ) -> Self {
        Self {
            kind,
            label,
            summary,
            category,
            factory,
        }
    }

    /// Constructs a fresh instance with default parameters.
    #[must_use]
    pub fn instantiate(&self) -> Box<dyn AudioEffect> {
        (self.factory)()
    }
}

impl fmt::Debug for EffectDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectDef")
            .field("kind", &self.kind)
            .field("label", &self.label)
            .field("category", &self.category)
            .finish_non_exhaustive()
    }
}

/// Finds a definition by its kind name.
#[must_use]
pub fn find_def<'a>(registry: &'a [EffectDef], kind: &str) -> Option<&'a EffectDef> {
    registry.iter().find(|def| def.kind.as_str() == kind)
}

/// Instantiates an effect from the registry.
///
/// # Errors
///
/// Returns [`CoreError::UnknownEffectKind`] when `kind` is not registered.
pub fn instantiate(registry: &[EffectDef], kind: &str) -> Result<Box<dyn AudioEffect>> {
    find_def(registry, kind)
        .map(EffectDef::instantiate)
        .ok_or_else(|| CoreError::UnknownEffectKind(kind.to_owned()))
}

/// The outcome of applying a batch of preset values to an effect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// How many parameters were written.
    pub applied: usize,
    /// Identifiers from the preset that the effect does not know about.
    ///
    /// Unknown identifiers are reported rather than rejected: a preset written
    /// by a newer version of the application should still load as far as it
    /// can.
    pub unknown: Vec<String>,
}

impl ApplyReport {
    /// Whether every identifier in the batch was recognised.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unknown.is_empty()
    }
}

/// Writes a batch of values onto a parameter set, matching by identifier.
///
/// Values are coerced to the type declared by the parameter, which keeps
/// hand-edited presets usable: writing `true` for a float parameter simply
/// yields `1.0`.
pub fn apply_parameter_pairs(set: &ParamSet, pairs: &[(ParamId, ParamValue)]) -> ApplyReport {
    let mut report = ApplyReport::default();
    for (id, value) in pairs {
        match set.index_of(id.as_str()) {
            Some(index) => {
                set.set_f32(index, value.to_f32());
                report.applied += 1;
            }
            None => report.unknown.push(id.to_string()),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param::{ParamSpec, ParamUnit};

    struct Passthrough {
        params: ParamSet,
    }

    impl Passthrough {
        fn new() -> Self {
            Self {
                params: ParamSet::new(vec![
                    ParamSpec::float("level", "Level", -20.0, 20.0, 0.0, 0.1, ParamUnit::Decibel),
                    ParamSpec::boolean("mute", "Mute", false),
                ]),
            }
        }
    }

    impl AudioEffect for Passthrough {
        fn kind(&self) -> EffectKind {
            EffectKind::new("test-passthrough")
        }
        fn parameters(&self) -> &ParamSet {
            &self.params
        }
        fn prepare(&mut self, _format: AudioFormat) {}
        fn reset(&mut self) {}
    }

    fn factory() -> Box<dyn AudioEffect> {
        Box::new(Passthrough::new())
    }

    static TEST_REGISTRY: [EffectDef; 1] = [EffectDef::new(
        EffectKind::new("test-passthrough"),
        "Test",
        "Does nothing.",
        EffectCategory::Utility,
        factory,
    )];

    #[test]
    fn registry_lookup_finds_and_misses() {
        assert!(find_def(&TEST_REGISTRY, "test-passthrough").is_some());
        assert!(find_def(&TEST_REGISTRY, "nope").is_none());
    }

    #[test]
    fn instantiation_reports_unknown_kinds() {
        let Err(error) = instantiate(&TEST_REGISTRY, "nope") else {
            panic!("an unregistered kind must not instantiate");
        };
        assert!(matches!(error, CoreError::UnknownEffectKind(_)));
        assert!(instantiate(&TEST_REGISTRY, "test-passthrough").is_ok());
    }

    #[test]
    fn applying_pairs_coerces_and_reports_unknown_ids() {
        let effect = Passthrough::new();
        let report = apply_parameter_pairs(
            effect.parameters(),
            &[
                (ParamId::new("level"), ParamValue::Float(-6.0)),
                (ParamId::new("mute"), ParamValue::Float(1.0)),
                (ParamId::new("ghost"), ParamValue::Float(1.0)),
            ],
        );
        assert_eq!(report.applied, 2);
        assert_eq!(report.unknown, vec!["ghost".to_owned()]);
        assert_eq!(effect.parameters().value_f32(0), -6.0);
        assert!(effect.parameters().value_bool(1));
    }
}
