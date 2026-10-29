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
mod tests {
    use super::*;
    use pwe_core::{ParamSpec, ParamUnit};

    #[test]
    fn knob_reads_live_values_and_falls_back() {
        let params = ParamSet::new(vec![
            ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
                .range(-12.0, 12.0, 3.0)
                .build(),
        ]);
        let knob = Knob::new(&params, "gain", 0.0);
        assert_eq!(knob.value(), 3.0);
        params.set_f32("gain", -6.0).unwrap();
        assert_eq!(knob.value(), -6.0);

        let missing = Knob::new(&params, "absent", 7.5);
        assert_eq!(missing.value(), 7.5);
    }
}
