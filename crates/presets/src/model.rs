use std::collections::BTreeMap;

use pwe_core::{ParamKind, ParamSpec, ParamValue};
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
                        return Err(PresetError::InvalidValue(format!(
                            "parameter '{}' is not a finite number",
                            spec.id
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub schema_version: u32,
    pub name: String,
    pub output: Pipeline,
}

impl Preset {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            output: Pipeline::default(),
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
mod tests {
    use super::*;
    use pwe_core::ParamSpec;

    fn spec() -> ParamSpec {
        ParamSpec::float("gain", "Gain", pwe_core::ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .build()
    }

    #[test]
    fn coercion_accepts_foreign_types() {
        let spec = spec();
        assert_eq!(
            PresetValue::Boolean(true).coerce(&spec).unwrap(),
            ParamValue::Float(1.0)
        );
        assert_eq!(
            PresetValue::Integer(4).coerce(&spec).unwrap(),
            ParamValue::Float(4.0)
        );
        assert_eq!(
            PresetValue::Text("3.5".into()).coerce(&spec).unwrap(),
            ParamValue::Float(3.5)
        );
    }

    #[test]
    fn coercion_rejects_non_finite_numbers() {
        assert!(PresetValue::Float(f64::INFINITY).coerce(&spec()).is_err());
    }

    #[test]
    fn text_matches_choice_labels() {
        let spec = ParamSpec::choice("mode", "Mode", &["Split", "Wideband"], 0);
        assert_eq!(
            PresetValue::Text("wideband".into()).coerce(&spec).unwrap(),
            ParamValue::Choice(1)
        );
    }

    #[test]
    fn round_trip_through_json() {
        let mut preset = Preset::new("Demo");
        let mut params = BTreeMap::new();
        params.insert("gain".to_owned(), PresetValue::Float(0.5));
        preset.output.effects.push(EffectEntry {
            kind: "gain".to_owned(),
            enabled: true,
            params,
        });
        let text = serde_json::to_string(&preset).unwrap();
        let parsed: Preset = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, preset);
    }

    #[test]
    fn names_are_validated() {
        assert!(validate_name("Night mode").is_ok());
        assert!(validate_name("../escape").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name(" hidden").is_err());
        assert!(validate_name(".hidden").is_err());
        assert!(validate_name("").is_err());
    }

    #[test]
    fn newer_schemas_are_rejected() {
        let mut preset = Preset::new("Future");
        preset.schema_version = SCHEMA_VERSION + 1;
        assert!(preset.validate().is_err());
    }
}
