use crate::param::value::ParamValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamUnit {
    None,
    Decibel,
    Hertz,
    Milliseconds,
    Percent,
    Ratio,
    Samples,
    Seconds,
}

impl ParamUnit {
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Decibel => "dB",
            Self::Hertz => "Hz",
            Self::Milliseconds => "ms",
            Self::Percent => "%",
            Self::Ratio => ":1",
            Self::Samples => "smp",
            Self::Seconds => "s",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamKind {
    Float {
        min: f32,
        max: f32,
        default: f32,
        step: f32,
        logarithmic: bool,
    },
    Boolean {
        default: bool,
    },
    Choice {
        labels: &'static [&'static str],
        default: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub unit: ParamUnit,
    pub kind: ParamKind,
}

impl ParamSpec {
    pub fn float(id: &'static str, label: &'static str, unit: ParamUnit) -> FloatParam {
        FloatParam {
            spec: ParamSpec {
                id,
                label,
                group: "",
                unit,
                kind: ParamKind::Float {
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    step: 0.0,
                    logarithmic: false,
                },
            },
        }
    }

    pub fn boolean(id: &'static str, label: &'static str, default: bool) -> Self {
        Self {
            id,
            label,
            group: "",
            unit: ParamUnit::None,
            kind: ParamKind::Boolean { default },
        }
    }

    pub fn choice(
        id: &'static str,
        label: &'static str,
        labels: &'static [&'static str],
        default: u16,
    ) -> Self {
        Self {
            id,
            label,
            group: "",
            unit: ParamUnit::None,
            kind: ParamKind::Choice {
                labels,
                default: default.min(labels.len().saturating_sub(1) as u16),
            },
        }
    }

    pub fn group(mut self, group: &'static str) -> Self {
        self.group = group;
        self
    }

    pub fn default_value(&self) -> ParamValue {
        match self.kind {
            ParamKind::Float { default, .. } => ParamValue::Float(default),
            ParamKind::Boolean { default } => ParamValue::Boolean(default),
            ParamKind::Choice { default, .. } => ParamValue::Choice(default),
        }
    }

    pub fn clamp_f32(&self, value: f32) -> f32 {
        match self.kind {
            ParamKind::Float { min, max, .. } => value.clamp(min, max),
            ParamKind::Boolean { .. } => {
                if value >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            ParamKind::Choice { labels, .. } => {
                let upper = labels.len().saturating_sub(1) as f32;
                value.round().clamp(0.0, upper)
            }
        }
    }

    pub fn denormalize(&self, normalised: f32) -> f32 {
        let ParamKind::Float {
            min,
            max,
            logarithmic,
            ..
        } = self.kind
        else {
            return self.clamp_f32(normalised);
        };
        let t = normalised.clamp(0.0, 1.0);
        if logarithmic && min > 0.0 && max > min {
            min * (max / min).powf(t)
        } else {
            min + t * (max - min)
        }
    }

    pub fn normalize(&self, value: f32) -> f32 {
        let ParamKind::Float {
            min,
            max,
            logarithmic,
            ..
        } = self.kind
        else {
            return self.clamp_f32(value);
        };
        let value = value.clamp(min, max);
        if logarithmic && min > 0.0 && max > min {
            (value / min).log(max / min)
        } else if max > min {
            (value - min) / (max - min)
        } else {
            0.0
        }
    }

    pub fn display(&self, value: ParamValue) -> String {
        match self.kind {
            ParamKind::Float { .. } => {
                let raw = value.as_f32();
                match self.unit {
                    ParamUnit::Hertz => format!("{raw:.0} Hz"),
                    ParamUnit::Milliseconds => format!("{raw:.1} ms"),
                    ParamUnit::Seconds => format!("{raw:.2} s"),
                    ParamUnit::Percent => format!("{raw:.0} %"),
                    ParamUnit::Ratio => format!("{raw:.2}:1"),
                    ParamUnit::Samples => format!("{raw:.0} smp"),
                    ParamUnit::Decibel => format!("{raw:+.1} dB"),
                    ParamUnit::None => format!("{raw:.2}"),
                }
            }
            ParamKind::Boolean { .. } => {
                if value.as_bool() {
                    "On".to_owned()
                } else {
                    "Off".to_owned()
                }
            }
            ParamKind::Choice { labels, .. } => {
                let index = value.as_choice() as usize;
                labels
                    .get(index)
                    .map_or_else(|| "Unknown".to_owned(), |label| (*label).to_owned())
            }
        }
    }
}

pub struct FloatParam {
    spec: ParamSpec,
}

impl FloatParam {
    pub fn range(mut self, min: f32, max: f32, default: f32) -> Self {
        if let ParamKind::Float {
            step, logarithmic, ..
        } = self.spec.kind
        {
            self.spec.kind = ParamKind::Float {
                min,
                max,
                default: default.clamp(min, max),
                step,
                logarithmic,
            };
        }
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        if let ParamKind::Float {
            min,
            max,
            default,
            logarithmic,
            ..
        } = self.spec.kind
        {
            self.spec.kind = ParamKind::Float {
                min,
                max,
                default,
                step,
                logarithmic,
            };
        }
        self
    }

    pub fn logarithmic(mut self) -> Self {
        if let ParamKind::Float {
            min,
            max,
            default,
            step,
            ..
        } = self.spec.kind
        {
            self.spec.kind = ParamKind::Float {
                min,
                max,
                default,
                step,
                logarithmic: true,
            };
        }
        self
    }

    pub fn group(mut self, group: &'static str) -> Self {
        self.spec.group = group;
        self
    }

    pub fn build(self) -> ParamSpec {
        self.spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logarithmic_normalisation_round_trips() {
        let spec = ParamSpec::float("frequency", "Frequency", ParamUnit::Hertz)
            .range(20.0, 20_000.0, 1_000.0)
            .logarithmic()
            .build();
        assert!((spec.normalize(632.46) - 0.5).abs() < 0.01);
        let value = spec.denormalize(0.5);
        assert!((value - 632.46).abs() < 1.0);
        assert!((spec.denormalize(0.0) - 20.0).abs() < 1.0e-3);
        assert!((spec.denormalize(1.0) - 20_000.0).abs() < 1.0e-1);
    }

    #[test]
    fn clamping_respects_bounds() {
        let spec = ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .build();
        assert!((spec.clamp_f32(100.0) - 20.0).abs() < 1.0e-6);
        assert!((spec.clamp_f32(-100.0) + 20.0).abs() < 1.0e-6);
    }

    #[test]
    fn choice_clamps_to_label_count() {
        let spec = ParamSpec::choice("mode", "Mode", &["a", "b"], 5);
        assert_eq!(spec.default_value(), ParamValue::Choice(1));
        assert_eq!(spec.clamp_f32(9.0), 1.0);
    }
}
