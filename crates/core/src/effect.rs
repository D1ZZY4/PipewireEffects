use std::fmt;

use crate::audio::{AudioBlock, AudioFormat};
use crate::error::{CoreError, Result};
use crate::param::{ParamId, ParamSet, ParamValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectKind(&'static str);

impl EffectKind {
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectCategory {
    Equalizer,
    Dynamics,
    Filter,
    Spatial,
    Utility,
}

impl EffectCategory {
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

pub trait AudioEffect: Send {
    fn kind(&self) -> EffectKind;

    fn parameters(&self) -> &ParamSet;

    fn prepare(&mut self, format: AudioFormat);

    fn reset(&mut self);

    #[allow(unused_variables)]
    fn process(&mut self, block: &mut AudioBlock<'_>) {}

    fn latency_frames(&self) -> usize {
        0
    }

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

pub type ResponseFn = fn(&ParamSet, f32, f32) -> Option<f32>;

#[derive(Clone, Copy)]
pub struct EffectDef {
    pub kind: EffectKind,
    pub label: &'static str,
    pub summary: &'static str,
    pub category: EffectCategory,
    pub factory: fn() -> Box<dyn AudioEffect>,
    pub response: Option<ResponseFn>,
}

impl EffectDef {
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
            response: None,
        }
    }

    #[must_use]
    pub const fn with_response(self, response: ResponseFn) -> Self {
        Self {
            kind: self.kind,
            label: self.label,
            summary: self.summary,
            category: self.category,
            factory: self.factory,
            response: Some(response),
        }
    }

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

#[must_use]
pub fn find_def<'a>(registry: &'a [EffectDef], kind: &str) -> Option<&'a EffectDef> {
    registry.iter().find(|def| def.kind.as_str() == kind)
}

pub fn instantiate(registry: &[EffectDef], kind: &str) -> Result<Box<dyn AudioEffect>> {
    find_def(registry, kind)
        .map(EffectDef::instantiate)
        .ok_or_else(|| CoreError::UnknownEffectKind(kind.to_owned()))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub applied: usize,
    pub unknown: Vec<String>,
}

impl ApplyReport {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unknown.is_empty()
    }
}

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
mod tests;
