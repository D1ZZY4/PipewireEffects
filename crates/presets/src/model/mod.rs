use std::collections::BTreeMap;

use pwe_core::i18n::tr;
use pwe_core::{ParamKind, ParamSpec, ParamValue, PipelineKind};
use serde::{Deserialize, Serialize};

use crate::error::{PresetError, PresetResult};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PresetValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

impl PresetValue {
    pub fn coerce(&self, spec: &ParamSpec) -> PresetResult<ParamValue> {
        let value = match spec.kind {
            ParamKind::Boolean { .. } => match self {
                Self::Boolean(flag) => ParamValue::Boolean(*flag),
                Self::Integer(number) => ParamValue::Boolean(*number != 0),
                Self::Float(number) => ParamValue::Boolean(*number != 0.0),
                Self::Text(text) => ParamValue::Boolean(text.eq_ignore_ascii_case("true")),
            },
            ParamKind::Choice { labels, default } => {
                let fallback =
                    ParamValue::Choice(default.min(labels.len().saturating_sub(1) as u16));
                let last = labels.len().saturating_sub(1) as f64;
                match self {
                    Self::Boolean(flag) => ParamValue::Choice(u16::from(*flag) * last as u16),
                    Self::Integer(number) => {
                        ParamValue::Choice((*number as f64).clamp(0.0, last) as u16)
                    }
                    Self::Float(number) => ParamValue::Choice(number.clamp(0.0, last) as u16),
                    Self::Text(text) => labels
                        .iter()
                        .position(|label| label.eq_ignore_ascii_case(text))
                        .map_or(fallback, |index| ParamValue::Choice(index as u16)),
                }
            }
            ParamKind::Float {
                min, max, default, ..
            } => match self {
                Self::Boolean(flag) => ParamValue::Float(if *flag { 1.0 } else { 0.0 }),
                Self::Integer(number) => ParamValue::Float(*number as f32),
                Self::Float(number) => {
                    if number.is_finite() {
                        ParamValue::Float(*number as f32)
                    } else {
                        return Err(PresetError::InvalidValue(tr(
                            "parameter '{0}' is not a finite number",
                            &[spec.id],
                        )));
                    }
                }
                Self::Text(text) => ParamValue::Float(text.parse::<f32>().unwrap_or(default)),
            }
            .clamped(min, max),
        };
        Ok(value.validate(spec)?)
    }

    pub fn from_param(value: ParamValue) -> Self {
        match value {
            ParamValue::Float(number) => {
                if number.fract() == 0.0 && number.abs() < 1.0e7 {
                    Self::Integer(number as i64)
                } else {
                    Self::Float(f64::from(number))
                }
            }
            ParamValue::Boolean(flag) => Self::Boolean(flag),
            ParamValue::Choice(index) => Self::Integer(i64::from(index)),
        }
    }
}

trait Clamped {
    fn clamped(self, min: f32, max: f32) -> Self;
}

impl Clamped for ParamValue {
    fn clamped(self, min: f32, max: f32) -> Self {
        match self {
            Self::Float(number) => Self::Float(number.clamp(min, max)),
            other => other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectEntry {
    pub kind: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub params: BTreeMap<String, PresetValue>,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Pipeline {
    #[serde(default)]
    pub bypassed: bool,
    #[serde(default)]
    pub effects: Vec<EffectEntry>,
}

impl Pipeline {
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty() && !self.bypassed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub schema_version: u32,
    pub name: String,
    pub output: Pipeline,
    #[serde(default)]
    pub input: Pipeline,
}

impl Preset {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            output: Pipeline::default(),
            input: Pipeline::default(),
        }
    }

    pub fn pipeline(&self, kind: PipelineKind) -> &Pipeline {
        match kind {
            PipelineKind::Output => &self.output,
            PipelineKind::Input => &self.input,
        }
    }

    pub fn pipeline_mut(&mut self, kind: PipelineKind) -> &mut Pipeline {
        match kind {
            PipelineKind::Output => &mut self.output,
            PipelineKind::Input => &mut self.input,
        }
    }

    pub fn validate(&self) -> PresetResult<()> {
        if self.schema_version > SCHEMA_VERSION {
            return Err(PresetError::UnsupportedVersion {
                found: self.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        validate_name(&self.name)
    }
}

pub fn validate_name(name: &str) -> PresetResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.len() > 96
        || trimmed != name
        || name.contains(['/', '\\'])
        || name.contains("..")
        || name.starts_with('.')
        || name.contains('\0')
    {
        return Err(PresetError::InvalidName(name.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
