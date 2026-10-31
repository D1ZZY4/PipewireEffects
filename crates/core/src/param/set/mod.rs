use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::{CoreError, CoreResult};
use crate::param::spec::{ParamKind, ParamSpec};
use crate::param::value::ParamValue;

#[derive(Debug)]
struct ParamStore {
    specs: Vec<ParamSpec>,
    values: Vec<AtomicU32>,
    index: HashMap<&'static str, usize>,
    defaults: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct ParamSet {
    inner: Arc<ParamStore>,
}

impl ParamSet {
    pub fn new(specs: Vec<ParamSpec>) -> Self {
        let mut index = HashMap::with_capacity(specs.len());
        let mut values = Vec::with_capacity(specs.len());
        let mut defaults = Vec::with_capacity(specs.len());
        for (position, spec) in specs.iter().enumerate() {
            index.insert(spec.id, position);
            let bits = spec.default_value().to_bits();
            values.push(AtomicU32::new(bits));
            defaults.push(bits);
        }
        Self {
            inner: Arc::new(ParamStore {
                specs,
                values,
                index,
                defaults,
            }),
        }
    }

    pub fn specs(&self) -> &[ParamSpec] {
        &self.inner.specs
    }

    pub fn len(&self) -> usize {
        self.inner.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.specs.is_empty()
    }

    pub fn spec(&self, id: &str) -> Option<&ParamSpec> {
        self.inner
            .index
            .get(id)
            .map(|position| &self.inner.specs[*position])
    }

    pub fn handle(&self, id: &'static str) -> Option<ParamHandle> {
        self.inner.index.get(id).map(|position| ParamHandle {
            store: Arc::clone(&self.inner),
            position: *position,
        })
    }

    pub fn get(&self, id: &str) -> Option<ParamValue> {
        let position = *self.inner.index.get(id)?;
        let spec = &self.inner.specs[position];
        let raw = f32::from_bits(self.inner.values[position].load(Ordering::Relaxed));
        Some(match spec.kind {
            ParamKind::Float { .. } => ParamValue::Float(raw),
            ParamKind::Boolean { .. } => ParamValue::Boolean(raw >= 0.5),
            ParamKind::Choice { .. } => ParamValue::Choice(raw.max(0.0) as u16),
        })
    }

    pub fn get_f32(&self, id: &str) -> Option<f32> {
        let position = *self.inner.index.get(id)?;
        Some(f32::from_bits(
            self.inner.values[position].load(Ordering::Relaxed),
        ))
    }

    pub fn set(&self, id: &str, value: ParamValue) -> CoreResult<ParamValue> {
        let position = *self
            .inner
            .index
            .get(id)
            .ok_or_else(|| CoreError::UnknownParameter(id.to_owned()))?;
        let spec = &self.inner.specs[position];
        let validated = value.validate(spec)?;
        self.inner.values[position].store(validated.to_bits(), Ordering::Relaxed);
        Ok(validated)
    }

    pub fn set_f32(&self, id: &str, value: f32) -> CoreResult<f32> {
        let validated = self.set(id, ParamValue::Float(value))?;
        Ok(validated.as_f32())
    }

    pub fn set_bool(&self, id: &str, value: bool) -> CoreResult<bool> {
        let validated = self.set(id, ParamValue::Boolean(value))?;
        Ok(validated.as_bool())
    }

    pub fn set_choice(&self, id: &str, index: u16) -> CoreResult<u16> {
        let validated = self.set(id, ParamValue::Choice(index))?;
        Ok(validated.as_choice())
    }

    pub fn reset(&self) {
        for (position, value) in self.inner.values.iter().enumerate() {
            value.store(self.inner.defaults[position], Ordering::Relaxed);
        }
    }

    pub fn entries(&self) -> Vec<(&'static str, ParamValue)> {
        self.inner
            .specs
            .iter()
            .enumerate()
            .map(|(position, spec)| {
                let raw = f32::from_bits(self.inner.values[position].load(Ordering::Relaxed));
                let value = match spec.kind {
                    ParamKind::Float { .. } => ParamValue::Float(raw),
                    ParamKind::Boolean { .. } => ParamValue::Boolean(raw >= 0.5),
                    ParamKind::Choice { .. } => ParamValue::Choice(raw.max(0.0) as u16),
                };
                (spec.id, value)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ParamHandle {
    store: Arc<ParamStore>,
    position: usize,
}

impl ParamHandle {
    pub fn spec(&self) -> &ParamSpec {
        &self.store.specs[self.position]
    }

    pub fn get_f32(&self) -> f32 {
        f32::from_bits(self.store.values[self.position].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
