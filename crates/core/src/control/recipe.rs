use crate::param::{ParamId, ParamValue};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ParamRepr {
    Bool(bool),
    Integer(i64),
    Number(f32),
}

impl ParamRepr {
    #[must_use]
    pub fn to_value(self) -> ParamValue {
        match self {
            Self::Bool(flag) => ParamValue::Bool(flag),
            Self::Integer(number) =>
            {
                #[allow(clippy::cast_precision_loss)]
                ParamValue::Float(number as f32)
            }
            Self::Number(number) => ParamValue::Float(number),
        }
    }
}

impl From<ParamValue> for ParamRepr {
    fn from(value: ParamValue) -> Self {
        match value {
            ParamValue::Bool(flag) => Self::Bool(flag),
            ParamValue::Enum(index) => Self::Integer(i64::try_from(index).unwrap_or(i64::MAX)),
            ParamValue::Float(number) => Self::Number(number),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectRecipe {
    pub kind: String,
    pub enabled: bool,
    pub values: Vec<(String, ParamRepr)>,
}

impl EffectRecipe {
    #[must_use]
    pub fn to_pairs(&self) -> Vec<(ParamId, ParamValue)> {
        self.values
            .iter()
            .map(|(id, repr)| (ParamId::new(id.clone()), repr.to_value()))
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChainRecipe {
    pub bypassed: bool,
    pub effects: Vec<EffectRecipe>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecipeReport {
    pub skipped: Vec<String>,
    pub unknown_params: Vec<String>,
}

impl RecipeReport {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.unknown_params.is_empty()
    }
}
