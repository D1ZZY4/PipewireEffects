use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::{ParamId, ParamKind, ParamSpec, ParamValue};
use crate::error::{CoreError, Result};

#[derive(Debug, Clone)]
pub struct AtomicParam {
    cell: Arc<AtomicU32>,
}

impl AtomicParam {
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            cell: Arc::new(AtomicU32::new(value.to_bits())),
        }
    }

    #[must_use]
    pub fn get(&self) -> f32 {
        f32::from_bits(self.cell.load(Ordering::Relaxed))
    }

    pub fn set(&self, value: f32) {
        self.cell.store(value.to_bits(), Ordering::Relaxed);
    }

    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cell, &other.cell)
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub spec: ParamSpec,
    pub value: AtomicParam,
}

#[derive(Debug, Clone)]
pub struct ParamSet {
    specs: Vec<ParamSpec>,
    values: Vec<AtomicParam>,
}

impl ParamSet {
    #[must_use]
    pub fn new(specs: Vec<ParamSpec>) -> Self {
        let values = specs
            .iter()
            .map(|spec| AtomicParam::new(spec.default_value().to_f32()))
            .collect();
        Self { specs, values }
    }

    #[must_use]
    pub fn specs(&self) -> &[ParamSpec] {
        &self.specs
    }

    #[must_use]
    pub fn values(&self) -> &[AtomicParam] {
        &self.values
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    #[must_use]
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.specs.iter().position(|spec| spec.id.as_str() == id)
    }

    #[must_use]
    pub fn spec(&self, index: usize) -> Option<&ParamSpec> {
        self.specs.get(index)
    }

    #[must_use]
    pub fn value(&self, index: usize) -> Option<ParamValue> {
        let spec = self.specs.get(index)?;
        let raw = self.values.get(index)?.get();
        Some(match spec.kind {
            ParamKind::Float { .. } => ParamValue::Float(raw),
            ParamKind::Bool { .. } => ParamValue::Bool(raw >= 0.5),
            ParamKind::Enum { .. } =>
            {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                ParamValue::Enum(raw.max(0.0) as usize)
            }
        })
    }

    #[must_use]
    pub fn value_f32(&self, index: usize) -> f32 {
        self.values.get(index).map_or(0.0, AtomicParam::get)
    }

    #[must_use]
    pub fn value_bool(&self, index: usize) -> bool {
        self.value_f32(index) >= 0.5
    }

    #[must_use]
    pub fn value_enum(&self, index: usize) -> usize {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            self.value_f32(index).max(0.0) as usize
        }
    }

    pub fn set(&self, index: usize, value: ParamValue) -> Result<()> {
        let spec = self.specs.get(index).ok_or(CoreError::IndexOutOfRange {
            index,
            len: self.specs.len(),
        })?;
        if !spec.kind.accepts(value) {
            return Err(CoreError::ValueTypeMismatch {
                param: spec.id.to_string(),
                expected: spec.kind.variant_name(),
                got: value.variant_name(),
            });
        }
        if matches!(spec.kind, ParamKind::Float { .. }) && !value.is_finite() {
            return Err(CoreError::ValueOutOfRange {
                param: spec.id.to_string(),
                value: value.to_f32(),
                min: spec.kind.bounds().map_or(0.0, |b| b.0),
                max: spec.kind.bounds().map_or(0.0, |b| b.1),
            });
        }
        if let Some(handle) = self.values.get(index) {
            handle.set(spec.kind.clamp_f32(value.to_f32()));
        }
        Ok(())
    }

    pub fn set_f32(&self, index: usize, value: f32) {
        if let (Some(spec), Some(handle)) = (self.specs.get(index), self.values.get(index)) {
            if value.is_finite() {
                handle.set(spec.kind.clamp_f32(value));
            }
        }
    }

    pub fn set_by_id(&self, id: &str, value: ParamValue) -> Result<()> {
        let index = self
            .index_of(id)
            .ok_or_else(|| CoreError::UnknownParameter {
                effect: String::new(),
                param: id.to_owned(),
            })?;
        self.set(index, value)
    }

    pub fn reset_to_defaults(&self) {
        for (spec, handle) in self.specs.iter().zip(&self.values) {
            handle.set(spec.default_value().to_f32());
        }
    }

    #[must_use]
    pub fn to_pairs(&self) -> Vec<(ParamId, ParamValue)> {
        self.specs
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| self.value(index).map(|value| (spec.id.clone(), value)))
            .collect()
    }

    #[must_use]
    pub fn format(&self, index: usize) -> String {
        match (self.specs.get(index), self.value(index)) {
            (Some(spec), Some(value)) => spec.kind.format_value(value),
            _ => String::from("—"),
        }
    }
}
