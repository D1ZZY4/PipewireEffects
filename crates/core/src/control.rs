use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;

use crate::audio::AudioFormat;
use crate::chain::{CHAIN_CAPACITY, ChainCommand};
use crate::effect::EffectCategory;
use crate::effect::{EffectDef, EffectKind, apply_parameter_pairs, find_def};

mod recipe;

use crate::error::{CoreError, Result};
use crate::param::ParamSet;
pub use recipe::{ChainRecipe, EffectRecipe, ParamRepr, RecipeReport};

pub const COMMAND_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub struct EffectModel {
    pub kind: EffectKind,
    pub label: String,
    pub category: EffectCategory,
    pub response: Option<crate::effect::ResponseFn>,
    pub enabled: bool,
    pub parameters: ParamSet,
}

impl EffectModel {
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

#[derive(Debug, Clone, Default)]
pub struct ChainModel {
    pub entries: Vec<EffectModel>,
    pub bypassed: bool,
}

impl ChainModel {
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&EffectModel> {
        self.entries.get(index)
    }

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

pub struct ChainController {
    commands: Arc<ArrayQueue<ChainCommand>>,
    registry: &'static [EffectDef],
    model: RefCell<ChainModel>,
    format: Cell<AudioFormat>,
}

impl ChainController {
    #[must_use]
    pub fn new(registry: &'static [EffectDef]) -> Self {
        Self {
            commands: Arc::new(ArrayQueue::new(COMMAND_CAPACITY)),
            registry,
            model: RefCell::new(ChainModel::default()),
            format: Cell::new(AudioFormat::default()),
        }
    }

    pub fn set_format(&self, format: AudioFormat) {
        self.format.set(format);
    }

    #[must_use]
    pub fn format(&self) -> AudioFormat {
        self.format.get()
    }

    #[must_use]
    pub fn command_queue(&self) -> Arc<ArrayQueue<ChainCommand>> {
        Arc::clone(&self.commands)
    }

    #[must_use]
    pub const fn registry(&self) -> &'static [EffectDef] {
        self.registry
    }

    pub fn with_model<T>(&self, f: impl FnOnce(&ChainModel) -> T) -> T {
        f(&self.model.borrow())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.model.borrow().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.model.borrow().is_empty()
    }

    #[must_use]
    pub fn is_bypassed(&self) -> bool {
        self.model.borrow().bypassed
    }

    #[must_use]
    pub fn to_recipe(&self) -> ChainRecipe {
        self.model.borrow().to_recipe()
    }

    fn post(&self, command: ChainCommand) -> Result<()> {
        self.commands
            .push(command)
            .map_err(|_| CoreError::CommandQueueFull)
    }

    pub fn add(&self, kind: &str) -> Result<usize> {
        self.insert(self.len(), kind)
    }

    pub fn insert(&self, index: usize, kind: &str) -> Result<usize> {
        let def = find_def(self.registry, kind)
            .ok_or_else(|| CoreError::UnknownEffectKind(kind.to_owned()))?;

        if self.len() >= CHAIN_CAPACITY {
            return Err(CoreError::ChainFull {
                capacity: CHAIN_CAPACITY,
            });
        }

        let mut effect = def.instantiate();
        effect.prepare(self.format.get());

        let model = EffectModel {
            kind: def.kind,
            label: def.label.to_owned(),
            category: def.category,
            response: def.response,
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

    pub fn insert_recipe(&self, index: usize, recipe: &EffectRecipe) -> Result<usize> {
        let def = find_def(self.registry, &recipe.kind)
            .ok_or_else(|| CoreError::UnknownEffectKind(recipe.kind.clone()))?;

        if self.len() >= CHAIN_CAPACITY {
            return Err(CoreError::ChainFull {
                capacity: CHAIN_CAPACITY,
            });
        }

        let mut effect = def.instantiate();
        apply_parameter_pairs(effect.parameters(), &recipe.to_pairs());
        effect.prepare(self.format.get());

        let model = EffectModel {
            kind: def.kind,
            label: def.label.to_owned(),
            category: def.category,
            response: def.response,
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

    pub fn set_all_enabled(&self, enabled: bool) -> Result<()> {
        self.post(ChainCommand::SetAllEnabled { enabled })?;
        for entry in &mut self.model.borrow_mut().entries {
            entry.enabled = enabled;
        }
        Ok(())
    }

    pub fn set_bypassed(&self, bypassed: bool) -> Result<()> {
        self.post(ChainCommand::SetBypassed { bypassed })?;
        self.model.borrow_mut().bypassed = bypassed;
        Ok(())
    }

    pub fn reset_effects(&self) -> Result<()> {
        self.post(ChainCommand::Reset)
    }

    pub fn clear(&self) -> Result<()> {
        self.post(ChainCommand::Clear)?;
        self.model.borrow_mut().entries.clear();
        Ok(())
    }

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
mod tests;
