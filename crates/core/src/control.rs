//! The interface-side model of the chain.
//!
//! [`ChainController`] is the single place where "what the chain should be"
//! lives. Every structural edit does two things, in this order:
//!
//! 1. posts a [`ChainCommand`] for the audio thread, and
//! 2. updates the local [`ChainModel`] that the interface renders from.
//!
//! Because both sides apply the same operations in the same order, they stay
//! in step without any shared lock.
//!
//! Parameter *values* never go through the queue. The model keeps a clone of
//! each effect's [`ParamSet`], and because a `ParamSet` clone shares the same
//! [`AtomicParam`](crate::param::AtomicParam) cells as the instance the audio
//! thread owns, moving a slider is immediately visible to the DSP. That is the
//! property to preserve when editing this file: **the instance that goes to the
//! audio thread and the parameters kept in the model must be the same
//! instance's cells.**
//!
//! The controller is deliberately not `Sync`. It belongs to the interface
//! thread, and its interior mutability is a `RefCell` rather than a mutex so
//! that a mistake fails loudly instead of becoming a priority inversion.

use std::cell::RefCell;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;

use crate::chain::ChainCommand;
use crate::effect::{EffectDef, EffectKind, apply_parameter_pairs, find_def};
use crate::error::{CoreError, Result};
use crate::param::{ParamId, ParamSet, ParamValue};

/// How many structural edits may be in flight before the audio thread catches
/// up. Structural edits happen at human speed, so this is generous.
pub const COMMAND_CAPACITY: usize = 256;

/// The interface-side description of one effect in the chain.
#[derive(Debug, Clone)]
pub struct EffectModel {
    /// The effect type.
    pub kind: EffectKind,
    /// Display name, copied from the registry so the model can be rendered
    /// without consulting the registry again.
    pub label: String,
    /// Whether the effect is processing audio.
    pub enabled: bool,
    /// The effect's parameters, sharing cells with the audio thread.
    pub parameters: ParamSet,
}

impl EffectModel {
    /// A compact description of the current settings, for the chain view.
    #[must_use]
    pub fn summary(&self) -> String {
        let parameters = &self.parameters;
        parameters
            .specs()
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| {
                parameters
                    .value(index)
                    .map(|value| spec.kind.format_value(value))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// The interface-side description of the whole chain.
#[derive(Debug, Clone, Default)]
pub struct ChainModel {
    /// The effects, in processing order.
    pub entries: Vec<EffectModel>,
    /// Whether the chain as a whole is bypassed.
    pub bypassed: bool,
}

impl ChainModel {
    /// The number of effects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry at `index`.
    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&EffectModel> {
        self.entries.get(index)
    }

    /// A portable description of the chain, suitable for saving as a preset.
    #[must_use]
    pub fn to_recipe(&self) -> ChainRecipe {
        ChainRecipe {
            bypassed: self.bypassed,
            effects: self
                .entries
                .iter()
                .map(|entry| EffectRecipe {
                    kind: entry.kind.as_str().to_owned(),
                    enabled: entry.enabled,
                    values: entry
                        .parameters
                        .to_pairs()
                        .into_iter()
                        .map(|(id, value)| (id.to_string(), ParamRepr::from(value)))
                        .collect(),
                })
                .collect(),
        }
    }
}

/// A parameter value in a form that survives a round trip through a preset file.
///
/// Serialised untagged, so a preset reads as plain JSON:
///
/// ```json
/// { "band1_gain": 6.0, "band1_enabled": true, "band1_type": 1 }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ParamRepr {
    /// A toggle.
    Bool(bool),
    /// A whole number, used for enumerations.
    Integer(i64),
    /// A continuous value.
    Number(f32),
}

impl ParamRepr {
    /// Converts to the domain value type.
    ///
    /// This is deliberately lossy about the *variant*: a preset file carries
    /// plain numbers, so an enumeration index comes back as
    /// [`ParamValue::Float`]. That is harmless, because the type is recovered
    /// from the target effect's [`ParamSpec`](crate::param::ParamSpec) when the
    /// value is written, and [`ParamValue::to_f32`] already means the same
    /// number for an enumeration index as it does for a float.
    #[must_use]
    pub fn to_value(self) -> ParamValue {
        match self {
            Self::Bool(flag) => ParamValue::Bool(flag),
            Self::Integer(number) =>
            {
                #[allow(clippy::cast_precision_loss)]
                ParamValue::Float(number as f32)
            }
            Self::Number(number) => ParamValue::Float(number),
        }
    }
}

impl From<ParamValue> for ParamRepr {
    fn from(value: ParamValue) -> Self {
        match value {
            ParamValue::Bool(flag) => Self::Bool(flag),
            ParamValue::Enum(index) => Self::Integer(i64::try_from(index).unwrap_or(i64::MAX)),
            ParamValue::Float(number) => Self::Number(number),
        }
    }
}

/// A portable description of one effect, used for preset round-tripping.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectRecipe {
    /// The effect type name.
    pub kind: String,
    /// Whether the effect is enabled.
    pub enabled: bool,
    /// Every parameter, keyed by identifier.
    pub values: Vec<(String, ParamRepr)>,
}

impl EffectRecipe {
    /// The values as domain pairs.
    #[must_use]
    pub fn to_pairs(&self) -> Vec<(ParamId, ParamValue)> {
        self.values
            .iter()
            .map(|(id, repr)| (ParamId::new(id.clone()), repr.to_value()))
            .collect()
    }
}

/// A portable description of a chain.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChainRecipe {
    /// Whether the chain as a whole is bypassed.
    pub bypassed: bool,
    /// The effects, in processing order.
    pub effects: Vec<EffectRecipe>,
}

/// What happened while applying a recipe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecipeReport {
    /// Effect kinds that are not registered and were skipped.
    pub skipped: Vec<String>,
    /// Parameter identifiers that no effect recognised.
    pub unknown_params: Vec<String>,
}

impl RecipeReport {
    /// Whether the recipe applied cleanly.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.unknown_params.is_empty()
    }
}

/// Owns the interface-side chain model and feeds the audio thread.
#[derive(Debug)]
pub struct ChainController {
    commands: Arc<ArrayQueue<ChainCommand>>,
    registry: &'static [EffectDef],
    model: RefCell<ChainModel>,
}

impl ChainController {
    /// Creates a controller backed by `registry`.
    #[must_use]
    pub fn new(registry: &'static [EffectDef]) -> Self {
        Self {
            commands: Arc::new(ArrayQueue::new(COMMAND_CAPACITY)),
            registry,
            model: RefCell::new(ChainModel::default()),
        }
    }

    /// The queue the audio thread drains. Hand this to the engine.
    #[must_use]
    pub fn command_queue(&self) -> Arc<ArrayQueue<ChainCommand>> {
        Arc::clone(&self.commands)
    }

    /// The registry this controller instantiates from.
    #[must_use]
    pub const fn registry(&self) -> &'static [EffectDef] {
        self.registry
    }

    /// Runs `f` against the current model.
    ///
    /// The model lives behind a `RefCell`, so `f` must not call back into the
    /// controller.
    pub fn with_model<T>(&self, f: impl FnOnce(&ChainModel) -> T) -> T {
        f(&self.model.borrow())
    }

    /// The number of effects in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.model.borrow().len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.model.borrow().is_empty()
    }

    /// Whether the whole chain is bypassed.
    #[must_use]
    pub fn is_bypassed(&self) -> bool {
        self.model.borrow().bypassed
    }

    /// Captures the chain as a portable recipe.
    #[must_use]
    pub fn to_recipe(&self) -> ChainRecipe {
        self.model.borrow().to_recipe()
    }

    fn post(&self, command: ChainCommand) -> Result<()> {
        self.commands
            .push(command)
            .map_err(|_| CoreError::CommandQueueFull)
    }

    /// Appends an effect to the end of the chain.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::UnknownEffectKind`] when `kind` is not registered,
    /// or [`CoreError::CommandQueueFull`] when the audio thread is not draining.
    pub fn add(&self, kind: &str) -> Result<usize> {
        self.insert(self.len(), kind)
    }

    /// Inserts an effect at `index`, clamped to the end of the chain.
    ///
    /// # Errors
    ///
    /// As [`ChainController::add`].
    pub fn insert(&self, index: usize, kind: &str) -> Result<usize> {
        let def = find_def(self.registry, kind)
            .ok_or_else(|| CoreError::UnknownEffectKind(kind.to_owned()))?;

        // The instance that goes to the audio thread, and the parameter cells
        // that the model keeps, must be one and the same.
        let effect = def.instantiate();
        let model = EffectModel {
            kind: def.kind,
            label: def.label.to_owned(),
            enabled: true,
            parameters: effect.parameters().clone(),
        };

        self.post(ChainCommand::Insert {
            index,
            effect,
            enabled: true,
        })?;

        let mut borrowed = self.model.borrow_mut();
        let index = index.min(borrowed.entries.len());
        borrowed.entries.insert(index, model);
        Ok(index)
    }

    /// Inserts a pre-built effect recipe at `index`.
    ///
    /// Used by preset loading, where the values must be written onto the
    /// instance *before* it is handed to the audio thread.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::UnknownEffectKind`] when the recipe names an
    /// unregistered effect, or [`CoreError::CommandQueueFull`] when the queue
    /// is saturated.
    pub fn insert_recipe(&self, index: usize, recipe: &EffectRecipe) -> Result<usize> {
        let def = find_def(self.registry, &recipe.kind)
            .ok_or_else(|| CoreError::UnknownEffectKind(recipe.kind.clone()))?;

        let effect = def.instantiate();
        apply_parameter_pairs(effect.parameters(), &recipe.to_pairs());

        let model = EffectModel {
            kind: def.kind,
            label: def.label.to_owned(),
            enabled: recipe.enabled,
            parameters: effect.parameters().clone(),
        };

        self.post(ChainCommand::Insert {
            index,
            effect,
            enabled: recipe.enabled,
        })?;

        let mut borrowed = self.model.borrow_mut();
        let index = index.min(borrowed.entries.len());
        borrowed.entries.insert(index, model);
        Ok(index)
    }

    /// Removes the effect at `index`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::IndexOutOfRange`] when `index` does not exist, or
    /// [`CoreError::CommandQueueFull`] when the queue is saturated.
    pub fn remove(&self, index: usize) -> Result<()> {
        {
            let model = self.model.borrow();
            if index >= model.entries.len() {
                return Err(CoreError::IndexOutOfRange {
                    index,
                    len: model.entries.len(),
                });
            }
        }
        self.post(ChainCommand::Remove { index })?;
        self.model.borrow_mut().entries.remove(index);
        Ok(())
    }

    /// Moves an effect to a new position.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::IndexOutOfRange`] when either index does not exist,
    /// or [`CoreError::CommandQueueFull`] when the queue is saturated.
    pub fn move_entry(&self, from: usize, to: usize) -> Result<()> {
        {
            let model = self.model.borrow();
            let len = model.entries.len();
            if from >= len || to >= len {
                return Err(CoreError::IndexOutOfRange {
                    index: from.max(to),
                    len,
                });
            }
        }
        self.post(ChainCommand::Move { from, to })?;
        if from != to {
            let mut model = self.model.borrow_mut();
            let entry = model.entries.remove(from);
            model.entries.insert(to, entry);
        }
        Ok(())
    }

    /// Enables or disables one effect.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::IndexOutOfRange`] when `index` does not exist, or
    /// [`CoreError::CommandQueueFull`] when the queue is saturated.
    pub fn set_enabled(&self, index: usize, enabled: bool) -> Result<()> {
        {
            let model = self.model.borrow();
            if model.entries.get(index).is_none() {
                return Err(CoreError::IndexOutOfRange {
                    index,
                    len: model.entries.len(),
                });
            }
        }
        self.post(ChainCommand::SetEnabled { index, enabled })?;
        if let Some(entry) = self.model.borrow_mut().entries.get_mut(index) {
            entry.enabled = enabled;
        }
        Ok(())
    }

    /// Enables or disables every effect.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::CommandQueueFull`] when the queue is saturated.
    pub fn set_all_enabled(&self, enabled: bool) -> Result<()> {
        self.post(ChainCommand::SetAllEnabled { enabled })?;
        for entry in &mut self.model.borrow_mut().entries {
            entry.enabled = enabled;
        }
        Ok(())
    }

    /// Bypasses or re-engages the whole chain.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::CommandQueueFull`] when the queue is saturated.
    pub fn set_bypassed(&self, bypassed: bool) -> Result<()> {
        self.post(ChainCommand::SetBypassed { bypassed })?;
        self.model.borrow_mut().bypassed = bypassed;
        Ok(())
    }

    /// Clears the filter memory of every effect.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::CommandQueueFull`] when the queue is saturated.
    pub fn reset_effects(&self) -> Result<()> {
        self.post(ChainCommand::Reset)
    }

    /// Removes every effect.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::CommandQueueFull`] when the queue is saturated.
    pub fn clear(&self) -> Result<()> {
        self.post(ChainCommand::Clear)?;
        self.model.borrow_mut().entries.clear();
        Ok(())
    }

    /// Replaces the whole chain with a recipe.
    ///
    /// Unregistered effect kinds and unrecognised parameter identifiers are
    /// reported rather than treated as fatal, so a preset written by another
    /// version of the application still applies as far as it can.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::CommandQueueFull`] when the queue is saturated.
    /// A recipe is a batch of commands, so a saturated queue can leave the
    /// chain partially applied; the caller should treat that as fatal.
    pub fn apply_recipe(&self, recipe: &ChainRecipe) -> Result<RecipeReport> {
        self.clear()?;

        let mut report = RecipeReport::default();
        let mut index = 0;
        for effect_recipe in &recipe.effects {
            if find_def(self.registry, &effect_recipe.kind).is_none() {
                report.skipped.push(effect_recipe.kind.clone());
                continue;
            }
            for unknown in self.collect_unknown_params(effect_recipe) {
                report.unknown_params.push(unknown);
            }
            self.insert_recipe(index, effect_recipe)?;
            index += 1;
        }

        self.set_bypassed(recipe.bypassed)?;
        Ok(report)
    }

    /// Reports parameter identifiers in a recipe that its effect does not know.
    fn collect_unknown_params(&self, recipe: &EffectRecipe) -> Vec<String> {
        let Some(def) = find_def(self.registry, &recipe.kind) else {
            return Vec::new();
        };
        let probe = def.instantiate();
        let parameters = probe.parameters();
        recipe
            .values
            .iter()
            .filter(|(id, _)| parameters.index_of(id).is_none())
            .map(|(id, _)| id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
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

        // Take the instance the audio thread would receive.
        let command = controller.command_queue().pop().unwrap();
        let ChainCommand::Insert { effect, .. } = command else {
            panic!("expected an insert command");
        };

        // Move the slider through the model.
        controller.with_model(|model| model.entries[0].parameters.set_f32(0, -12.0));

        // The audio thread's instance must see it.
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
        // A preset file stores numbers, so enumerations come back as floats.
        // What has to survive is the number, because that is what the target
        // parameter's specification clamps and interprets.
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
}
