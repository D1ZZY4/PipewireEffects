use std::fmt;
mod set;
mod spec;
mod value;

pub use set::{AtomicParam, Param, ParamSet};
pub use spec::ParamSpec;
pub use value::ParamValue;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParamId(String);

impl ParamId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ParamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ParamId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ParamId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl PartialEq<str> for ParamId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParamUnit {
    #[default]
    None,
    Decibel,
    Hertz,
    Percent,
    Seconds,
    Milliseconds,
    Ratio,
    Quality,
}

impl ParamUnit {
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Decibel => " dB",
            Self::Hertz => " Hz",
            Self::Percent => " %",
            Self::Seconds => " s",
            Self::Milliseconds => " ms",
            Self::Ratio => ":1",
            Self::Quality => "",
        }
    }
}

#[must_use]
const fn decimals_for(step: f32) -> usize {
    if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else {
        2
    }
}

#[must_use]
fn format_hertz(value: f32) -> String {
    if value >= 1_000.0 {
        format!("{:.2} kHz", value / 1_000.0)
    } else {
        format!("{value:.0} Hz")
    }
}

#[must_use]
fn snap_to_step(value: f32, step: f32) -> f32 {
    if step <= 0.0 {
        value
    } else {
        (value / step).round() * step
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    Float {
        min: f32,
        max: f32,
        default: f32,
        step: f32,
        unit: ParamUnit,
        logarithmic: bool,
    },
    Bool {
        default: bool,
    },
    Enum {
        options: Vec<String>,
        default: usize,
    },
}

impl ParamKind {
    #[must_use]
    pub fn default_value(&self) -> ParamValue {
        match self {
            Self::Float { default, .. } => ParamValue::Float(*default),
            Self::Bool { default } => ParamValue::Bool(*default),
            Self::Enum { default, .. } => ParamValue::Enum(*default),
        }
    }

    #[must_use]
    pub fn accepts(&self, value: ParamValue) -> bool {
        matches!(
            (self, value),
            (Self::Float { .. }, ParamValue::Float(_))
                | (Self::Bool { .. }, ParamValue::Bool(_))
                | (Self::Enum { .. }, ParamValue::Enum(_))
        )
    }

    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::Float { .. } => "float",
            Self::Bool { .. } => "boolean",
            Self::Enum { .. } => "enumeration",
        }
    }

    #[must_use]
    pub const fn logarithmic(&self) -> bool {
        matches!(
            self,
            Self::Float {
                logarithmic: true,
                ..
            }
        )
    }

    #[must_use]
    pub fn step(&self) -> f32 {
        match self {
            Self::Float { step, .. } => *step,
            Self::Bool { .. } | Self::Enum { .. } => 1.0,
        }
    }

    #[must_use]
    pub const fn bounds(&self) -> Option<(f32, f32)> {
        match self {
            Self::Float { min, max, .. } => Some((*min, *max)),
            Self::Bool { .. } | Self::Enum { .. } => None,
        }
    }

    #[must_use]
    pub fn option_count(&self) -> Option<usize> {
        match self {
            Self::Enum { options, .. } => Some(options.len()),
            _ => None,
        }
    }

    #[must_use]
    pub fn clamp_f32(&self, value: f32) -> f32 {
        match self {
            Self::Float { min, max, .. } => value.clamp(*min, *max),
            Self::Bool { .. } => {
                if value >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Enum { options, .. } => {
                let last = options.len().saturating_sub(1);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = value.max(0.0) as usize;
                #[allow(clippy::cast_precision_loss)]
                {
                    index.min(last) as f32
                }
            }
        }
    }

    #[must_use]
    pub fn normalise(&self, value: f32) -> f64 {
        match self {
            Self::Float {
                min,
                max,
                logarithmic,
                ..
            } => {
                let span = max - min;
                if span <= 0.0 {
                    return 0.0;
                }
                if *logarithmic && *min > 0.0 {
                    let low = f64::from(*min).ln();
                    let high = f64::from(*max).ln();
                    ((f64::from(value.max(0.0).max(*min)).ln() - low) / (high - low))
                        .clamp(0.0, 1.0)
                } else {
                    f64::from((value - min) / span).clamp(0.0, 1.0)
                }
            }
            Self::Bool { .. } => f64::from(value >= 0.5),
            Self::Enum { options, .. } => {
                let last = options.len().saturating_sub(1);
                if last == 0 {
                    0.0
                } else {
                    #[allow(clippy::cast_precision_loss)]
                    let position = f64::from(value.max(0.0) / last as f32);
                    position.clamp(0.0, 1.0)
                }
            }
        }
    }

    #[must_use]
    pub fn denormalise(&self, position: f64) -> f32 {
        let position = position.clamp(0.0, 1.0);
        match self {
            Self::Float {
                min,
                max,
                logarithmic,
                ..
            } => {
                if *logarithmic && *min > 0.0 {
                    let low = f64::from(*min).ln();
                    let high = f64::from(*max).ln();
                    #[allow(clippy::cast_possible_truncation)]
                    let value = (low + position * (high - low)).exp() as f32;
                    value.clamp(*min, *max)
                } else {
                    #[allow(clippy::cast_possible_truncation)]
                    let value = (*min as f64 + position * f64::from(*max - *min)) as f32;
                    value.clamp(*min, *max)
                }
            }
            Self::Bool { .. } => {
                if position >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Enum { options, .. } => {
                let last = options.len().saturating_sub(1);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = (position * last as f64).round() as usize;
                #[allow(clippy::cast_precision_loss)]
                {
                    index.min(last) as f32
                }
            }
        }
    }

    #[must_use]
    pub fn format_value(&self, value: ParamValue) -> String {
        match (self, value) {
            (Self::Float { unit, step, .. }, ParamValue::Float(raw)) => {
                let snapped = snap_to_step(raw, *step);
                match unit {
                    ParamUnit::Hertz => format_hertz(snapped),
                    ParamUnit::Ratio => format!("{snapped:.1}{}", unit.suffix()),
                    ParamUnit::None => format!("{snapped:.prec$}", prec = decimals_for(*step)),
                    _ => format!(
                        "{snapped:.prec$}{}",
                        unit.suffix(),
                        prec = decimals_for(*step)
                    ),
                }
            }
            (Self::Bool { .. }, ParamValue::Bool(raw)) => {
                (if raw { "On" } else { "Off" }).to_owned()
            }
            (Self::Enum { options, .. }, ParamValue::Enum(index)) => options
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("#{index}")),
            (_, other) => format!("{other:?}"),
        }
    }
}

#[cfg(test)]
mod tests;
