use super::{ParamId, ParamKind, ParamUnit, ParamValue};

#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    pub id: ParamId,
    pub label: String,
    pub kind: ParamKind,
    pub doc: String,
}

impl ParamSpec {
    pub fn new(id: impl Into<ParamId>, label: impl Into<String>, kind: ParamKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            doc: String::new(),
        }
    }

    pub fn float(
        id: impl Into<ParamId>,
        label: impl Into<String>,
        min: f32,
        max: f32,
        default: f32,
        step: f32,
        unit: ParamUnit,
    ) -> Self {
        Self::new(
            id,
            label,
            ParamKind::Float {
                min,
                max,
                default: default.clamp(min, max),
                step,
                unit,
                logarithmic: false,
            },
        )
    }

    pub fn float_log(
        id: impl Into<ParamId>,
        label: impl Into<String>,
        min: f32,
        max: f32,
        default: f32,
        unit: ParamUnit,
    ) -> Self {
        Self::new(
            id,
            label,
            ParamKind::Float {
                min,
                max,
                default: default.clamp(min, max),
                step: 0.0,
                unit,
                logarithmic: true,
            },
        )
    }

    pub fn boolean(id: impl Into<ParamId>, label: impl Into<String>, default: bool) -> Self {
        Self::new(id, label, ParamKind::Bool { default })
    }

    pub fn enumerate(
        id: impl Into<ParamId>,
        label: impl Into<String>,
        options: &[&str],
        default: usize,
    ) -> Self {
        let options: Vec<String> = options.iter().map(|o| (*o).to_owned()).collect();
        let default = default.min(options.len().saturating_sub(1));
        Self::new(id, label, ParamKind::Enum { options, default })
    }

    #[must_use]
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    #[must_use]
    pub fn default_value(&self) -> ParamValue {
        self.kind.default_value()
    }
}
