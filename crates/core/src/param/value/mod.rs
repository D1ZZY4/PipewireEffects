use crate::error::{CoreError, CoreResult};
use crate::param::spec::{ParamKind, ParamSpec};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    Float(f32),
    Boolean(bool),
    Choice(u16),
}

impl ParamValue {
    pub fn as_f32(self) -> f32 {
        match self {
            Self::Float(value) => value,
            Self::Boolean(value) => {
                if value {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Choice(index) => f32::from(index),
        }
    }

    pub fn as_bool(self) -> bool {
        match self {
            Self::Boolean(value) => value,
            Self::Float(value) => value >= 0.5,
            Self::Choice(index) => index != 0,
        }
    }

    pub fn as_choice(self) -> u16 {
        match self {
            Self::Choice(index) => index,
            Self::Boolean(value) => u16::from(value),
            Self::Float(value) => value.round().max(0.0) as u16,
        }
    }

    pub fn to_bits(self) -> u32 {
        self.as_f32().to_bits()
    }

    pub fn validate(self, spec: &ParamSpec) -> CoreResult<Self> {
        match spec.kind {
            ParamKind::Float { .. } => {
                let value = self.as_f32();
                if value.is_finite() {
                    Ok(Self::Float(spec.clamp_f32(value)))
                } else {
                    Err(CoreError::InvalidValue {
                        id: spec.id.to_owned(),
                        detail: crate::i18n::t("value is not finite").to_owned(),
                    })
                }
            }
            ParamKind::Boolean { .. } => Ok(Self::Boolean(self.as_bool())),
            ParamKind::Choice { labels, .. } => {
                let index = self.as_choice();
                if labels.is_empty() {
                    return Err(CoreError::InvalidValue {
                        id: spec.id.to_owned(),
                        detail: crate::i18n::t("parameter has no selectable options").to_owned(),
                    });
                }
                Ok(Self::Choice(index.min(labels.len() as u16 - 1)))
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
