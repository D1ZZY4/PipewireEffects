use pwe_core::{ParamHandle, ParamSet};

#[derive(Debug, Clone)]
pub struct Knob {
    handle: Option<ParamHandle>,
    fallback: f32,
}

impl Knob {
    pub fn new(params: &ParamSet, id: &'static str, fallback: f32) -> Self {
        let default = params
            .spec(id)
            .map_or(fallback, |spec| spec.default_value().as_f32());
        Self {
            handle: params.handle(id),
            fallback: default,
        }
    }

    pub fn value(&self) -> f32 {
        match &self.handle {
            Some(handle) => handle.get_f32(),
            None => self.fallback,
        }
    }

    pub fn flag(&self) -> bool {
        self.value() >= 0.5
    }

    pub fn index(&self) -> usize {
        self.value().max(0.0) as usize
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
