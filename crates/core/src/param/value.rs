#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    Float(f32),
    Bool(bool),
    Enum(usize),
}

impl ParamValue {
    #[must_use]
    pub fn to_f32(self) -> f32 {
        match self {
            Self::Float(value) => value,
            Self::Bool(value) => {
                if value {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Enum(index) => {
                #[allow(clippy::cast_precision_loss)]
                {
                    index as f32
                }
            }
        }
    }

    #[must_use]
    pub fn to_bool(self) -> bool {
        match self {
            Self::Bool(value) => value,
            Self::Float(value) => value >= 0.5,
            Self::Enum(index) => index != 0,
        }
    }

    #[must_use]
    pub fn to_index(self) -> usize {
        match self {
            Self::Enum(index) => index,
            Self::Float(value) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    value.max(0.0) as usize
                }
            }
            Self::Bool(value) => usize::from(value),
        }
    }

    #[must_use]
    pub fn is_finite(self) -> bool {
        match self {
            Self::Float(value) => value.is_finite(),
            Self::Bool(_) | Self::Enum(_) => true,
        }
    }

    #[must_use]
    pub const fn variant_name(self) -> &'static str {
        match self {
            Self::Float(_) => "float",
            Self::Bool(_) => "boolean",
            Self::Enum(_) => "enumeration",
        }
    }
}
