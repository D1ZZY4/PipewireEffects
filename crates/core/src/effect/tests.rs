use super::*;
use crate::param::{ParamSpec, ParamUnit};

struct Passthrough {
    params: ParamSet,
}

impl Passthrough {
    fn new() -> Self {
        Self {
            params: ParamSet::new(vec![
                ParamSpec::float("level", "Level", -20.0, 20.0, 0.0, 0.1, ParamUnit::Decibel),
                ParamSpec::boolean("mute", "Mute", false),
            ]),
        }
    }
}

impl AudioEffect for Passthrough {
    fn kind(&self) -> EffectKind {
        EffectKind::new("test-passthrough")
    }
    fn parameters(&self) -> &ParamSet {
        &self.params
    }
    fn prepare(&mut self, _format: AudioFormat) {}
    fn reset(&mut self) {}
}

fn factory() -> Box<dyn AudioEffect> {
    Box::new(Passthrough::new())
}

static TEST_REGISTRY: [EffectDef; 1] = [EffectDef::new(
    EffectKind::new("test-passthrough"),
    "Test",
    "Does nothing.",
    EffectCategory::Utility,
    factory,
)];

#[test]
fn registry_lookup_finds_and_misses() {
    assert!(find_def(&TEST_REGISTRY, "test-passthrough").is_some());
    assert!(find_def(&TEST_REGISTRY, "nope").is_none());
}

#[test]
fn instantiation_reports_unknown_kinds() {
    let Err(error) = instantiate(&TEST_REGISTRY, "nope") else {
        panic!("an unregistered kind must not instantiate");
    };
    assert!(matches!(error, CoreError::UnknownEffectKind(_)));
    assert!(instantiate(&TEST_REGISTRY, "test-passthrough").is_ok());
}

#[test]
fn applying_pairs_coerces_and_reports_unknown_ids() {
    let effect = Passthrough::new();
    let report = apply_parameter_pairs(
        effect.parameters(),
        &[
            (ParamId::new("level"), ParamValue::Float(-6.0)),
            (ParamId::new("mute"), ParamValue::Float(1.0)),
            (ParamId::new("ghost"), ParamValue::Float(1.0)),
        ],
    );
    assert_eq!(report.applied, 2);
    assert_eq!(report.unknown, vec!["ghost".to_owned()]);
    assert_eq!(effect.parameters().value_f32(0), -6.0);
    assert!(effect.parameters().value_bool(1));
}
