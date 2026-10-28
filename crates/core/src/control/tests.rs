use crate::param::ParamValue;

use super::*;
use crate::audio::{AudioBlock, AudioFormat};
use crate::effect::EffectCategory;
use crate::param::{ParamSpec, ParamUnit};

struct TestEffect {
    parameters: ParamSet,
}

impl TestEffect {
    fn new() -> Self {
        Self {
            parameters: ParamSet::new(vec![
                ParamSpec::float("gain", "Gain", -60.0, 24.0, 0.0, 0.1, ParamUnit::Decibel),
                ParamSpec::boolean("mute", "Mute", false),
            ]),
        }
    }
}

impl crate::effect::AudioEffect for TestEffect {
    fn kind(&self) -> EffectKind {
        EffectKind::new("test")
    }
    fn parameters(&self) -> &ParamSet {
        &self.parameters
    }
    fn prepare(&mut self, _format: AudioFormat) {}
    fn reset(&mut self) {}
    fn process(&mut self, block: &mut AudioBlock<'_>) {
        if self.parameters.value_bool(1) {
            block.fill_silence();
        }
    }
}

fn factory() -> Box<dyn crate::effect::AudioEffect> {
    Box::new(TestEffect::new())
}

static REGISTRY: [EffectDef; 1] = [EffectDef::new(
    EffectKind::new("test"),
    "Test",
    "A test effect.",
    EffectCategory::Utility,
    factory,
)];

fn make_controller() -> ChainController {
    ChainController::new(&REGISTRY)
}

#[test]
fn adding_updates_the_model_and_queues_a_command() {
    let controller = make_controller();
    let index = controller.add("test").unwrap();
    assert_eq!(index, 0);
    assert_eq!(controller.len(), 1);
    assert_eq!(controller.command_queue().len(), 1);
}

#[test]
fn the_model_shares_parameter_cells_with_the_queued_instance() {
    let controller = make_controller();
    controller.add("test").unwrap();

    let command = controller.command_queue().pop().unwrap();
    let ChainCommand::Insert { effect, .. } = command else {
        panic!("expected an insert command");
    };

    controller.with_model(|model| model.entries[0].parameters.set_f32(0, -12.0));

    assert_eq!(effect.parameters().value_f32(0), -12.0);
}

#[test]
fn unknown_effect_kinds_are_rejected() {
    let controller = make_controller();
    assert!(matches!(
        controller.add("nope"),
        Err(CoreError::UnknownEffectKind(_))
    ));
    assert!(controller.is_empty());
}

#[test]
fn removing_an_out_of_range_index_is_an_error_and_changes_nothing() {
    let controller = make_controller();
    controller.add("test").unwrap();
    assert!(matches!(
        controller.remove(5),
        Err(CoreError::IndexOutOfRange { .. })
    ));
    assert_eq!(controller.len(), 1);
}

#[test]
fn moving_entries_keeps_model_and_commands_in_the_same_order() {
    let controller = make_controller();
    controller.add("test").unwrap();
    controller.add("test").unwrap();

    controller.move_entry(0, 1).unwrap();
    let commands: Vec<ChainCommand> = (0..3)
        .filter_map(|_| controller.command_queue().pop())
        .collect();
    assert!(matches!(commands[0], ChainCommand::Insert { index: 0, .. }));
    assert!(matches!(commands[1], ChainCommand::Insert { index: 1, .. }));
    assert!(matches!(commands[2], ChainCommand::Move { from: 0, to: 1 }));
    assert_eq!(controller.len(), 2);
}

#[test]
fn a_recipe_round_trips_through_the_model() {
    let controller = make_controller();
    controller.add("test").unwrap();
    controller.set_enabled(0, false).unwrap();
    controller.with_model(|model| {
        model.entries[0].parameters.set_f32(0, 7.5);
    });

    let recipe = controller.to_recipe();
    assert_eq!(recipe.effects.len(), 1);
    assert!(!recipe.effects[0].enabled);

    let fresh = make_controller();
    let report = fresh.apply_recipe(&recipe).unwrap();
    assert!(report.is_complete());
    fresh.with_model(|model| {
        assert_eq!(model.entries.len(), 1);
        assert!((model.entries[0].parameters.value_f32(0) - 7.5).abs() < 1e-6);
        assert!(!model.entries[0].enabled);
    });
}

#[test]
fn a_recipe_with_unknown_kinds_and_params_reports_rather_than_fails() {
    let controller = make_controller();
    let recipe = ChainRecipe {
        bypassed: false,
        effects: vec![
            EffectRecipe {
                kind: "ghost".to_owned(),
                enabled: true,
                values: Vec::new(),
            },
            EffectRecipe {
                kind: "test".to_owned(),
                enabled: true,
                values: vec![("nonsense".to_owned(), ParamRepr::Number(1.0))],
            },
        ],
    };
    let report = controller.apply_recipe(&recipe).unwrap();
    assert_eq!(report.skipped, vec!["ghost".to_owned()]);
    assert_eq!(report.unknown_params, vec!["nonsense".to_owned()]);
    assert_eq!(controller.len(), 1);
}

#[test]
fn applying_a_recipe_replaces_the_previous_chain() {
    let controller = make_controller();
    controller.add("test").unwrap();
    controller.add("test").unwrap();

    let recipe = ChainRecipe {
        bypassed: false,
        effects: vec![EffectRecipe {
            kind: "test".to_owned(),
            enabled: true,
            values: Vec::new(),
        }],
    };
    controller.apply_recipe(&recipe).unwrap();
    assert_eq!(controller.len(), 1);
}

#[test]
fn bypass_flag_is_carried_to_the_model() {
    let controller = make_controller();
    controller.set_bypassed(true).unwrap();
    assert!(controller.is_bypassed());
    assert!(matches!(
        controller.command_queue().pop().unwrap(),
        ChainCommand::SetBypassed { bypassed: true }
    ));
}

#[test]
fn clear_empties_the_model() {
    let controller = make_controller();
    controller.add("test").unwrap();
    controller.clear().unwrap();
    assert!(controller.is_empty());
}

#[test]
fn parameter_repr_preserves_the_numeric_value_rather_than_the_variant() {
    for value in [
        ParamValue::Bool(true),
        ParamValue::Enum(3),
        ParamValue::Float(-4.25),
    ] {
        assert_eq!(ParamRepr::from(value).to_value().to_f32(), value.to_f32());
    }
    assert_eq!(
        ParamRepr::from(ParamValue::Bool(false)).to_value(),
        ParamValue::Bool(false)
    );
}
