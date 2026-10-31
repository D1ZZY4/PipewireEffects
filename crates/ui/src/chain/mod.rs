use std::collections::BTreeMap;

use pwe_core::ParamSet;
use pwe_core::i18n::tr;
use pwe_engine::{EffectSlot, EngineHandle};
use pwe_presets::{EffectEntry, Pipeline, PresetError, PresetValue};

pub struct Slot {
    pub id: EffectSlot,
    pub kind: &'static str,
    pub params: ParamSet,
    pub enabled: bool,
}

pub struct Chain {
    engine: Option<EngineHandle>,
    pub slots: Vec<Slot>,
    pub selected: Option<EffectSlot>,
    pub bypassed: bool,
    next_slot: u64,
}

impl Chain {
    pub fn new(engine: Option<EngineHandle>) -> Self {
        Self {
            engine,
            slots: Vec::new(),
            selected: None,
            bypassed: false,
            next_slot: 1,
        }
    }

    pub fn engine_mut(&mut self) -> Option<&mut EngineHandle> {
        self.engine.as_mut()
    }

    pub fn running(&self) -> bool {
        self.engine.is_some()
    }

    pub fn engine(&self) -> Option<&EngineHandle> {
        self.engine.as_ref()
    }

    pub fn start(&mut self, engine: EngineHandle) {
        self.engine = Some(engine);
        self.reapply();
    }

    pub fn stop(&mut self) {
        self.engine = None;
        self.selected = None;
    }

    pub fn max_effects(&self) -> usize {
        self.engine
            .as_ref()
            .map_or(pwe_engine::MAX_EFFECTS, EngineHandle::max_effects)
    }

    pub fn add(&mut self, kind: &str) -> Option<EffectSlot> {
        if self.slots.len() >= self.max_effects() {
            return None;
        }
        let descriptor = pwe_dsp::find(kind)?;
        let (params, effect) = pwe_dsp::instantiate(kind)?;
        let id = EffectSlot(self.next_slot);
        self.next_slot += 1;
        if let Some(engine) = &self.engine
            && engine.add_effect(id, effect).is_err()
        {
            return None;
        }
        self.slots.push(Slot {
            id,
            kind: descriptor.id,
            params,
            enabled: true,
        });
        self.selected = Some(id);
        Some(id)
    }

    pub fn remove(&mut self, id: EffectSlot) {
        if let Some(index) = self.slots.iter().position(|slot| slot.id == id) {
            let slot = self.slots.remove(index);
            if let Some(engine) = &self.engine {
                let _ = engine.remove_effect(slot.id);
            }
            if self.selected == Some(id) {
                self.selected = self.slots.get(index.saturating_sub(1)).map(|slot| slot.id);
            }
        }
    }

    pub fn move_by(&mut self, id: EffectSlot, delta: isize) {
        let Some(index) = self.slots.iter().position(|slot| slot.id == id) else {
            return;
        };
        let target = (index as isize + delta).clamp(0, self.slots.len() as isize - 1) as usize;
        if target == index {
            return;
        }
        let slot = self.slots.remove(index);
        self.slots.insert(target, slot);
        if let Some(engine) = &self.engine {
            let _ = engine.move_effect(id, target);
        }
    }

    pub fn set_enabled(&mut self, id: EffectSlot, enabled: bool) {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) {
            slot.enabled = enabled;
            if let Some(engine) = &self.engine {
                let _ = engine.set_effect_enabled(id, enabled);
            }
        }
    }

    pub fn set_bypass(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
        if let Some(engine) = &self.engine {
            let _ = engine.set_bypass(bypassed);
        }
    }

    pub fn set_target_device(&self, target: impl Into<String>) {
        if let Some(engine) = &self.engine {
            let _ = engine.set_target_device(target);
        }
    }

    pub fn slot(&self, id: EffectSlot) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.id == id)
    }

    pub fn clear(&mut self) {
        for slot in std::mem::take(&mut self.slots) {
            if let Some(engine) = &self.engine {
                let _ = engine.remove_effect(slot.id);
            }
        }
        self.selected = None;
    }

    pub fn apply(&mut self, section: &Pipeline, warnings: &mut Vec<String>) {
        let was_bypassed = self.bypassed;
        self.clear();
        for entry in &section.effects {
            match self.add_entry(entry, warnings) {
                Some(id) => {
                    if !entry.enabled {
                        self.set_enabled(id, false);
                    }
                    self.selected = Some(id);
                }
                None => warnings.push(tr("'{0}' is not available", &[&entry.kind])),
            }
        }
        if section.bypassed != was_bypassed || !self.slots.is_empty() {
            self.set_bypass(section.bypassed);
        }
    }

    pub fn snapshot(&self, section: &mut Pipeline) {
        section.bypassed = self.bypassed;
        section.effects.clear();
        for slot in &self.slots {
            let mut params = BTreeMap::new();
            for (id, value) in slot.params.entries() {
                if let Some(spec) = slot.params.spec(id)
                    && spec.default_value() != value
                {
                    params.insert(id.to_owned(), PresetValue::from_param(value));
                }
            }
            section.effects.push(EffectEntry {
                kind: slot.kind.to_owned(),
                enabled: slot.enabled,
                params,
            });
        }
    }

    pub fn reapply(&mut self) {
        let Some(engine) = &self.engine else {
            return;
        };
        for slot in &self.slots {
            if let Some(effect) = pwe_dsp::rebuild(slot.kind, slot.params.clone()) {
                let _ = engine.add_effect(slot.id, effect);
            }
            if !slot.enabled {
                let _ = engine.set_effect_enabled(slot.id, false);
            }
        }
        let _ = engine.set_bypass(self.bypassed);
        let highest = self.slots.iter().map(|slot| slot.id.0).max().unwrap_or(0);
        self.next_slot = self.next_slot.max(highest + 1);
    }

    fn add_entry(&mut self, entry: &EffectEntry, warnings: &mut Vec<String>) -> Option<EffectSlot> {
        let descriptor = pwe_dsp::find(&entry.kind)?;
        let (params, effect) = pwe_dsp::instantiate(&entry.kind)?;
        for (id, value) in &entry.params {
            let Some(spec) = params.spec(id) else {
                warnings.push(tr("'{0}' has no parameter '{1}'", &[&entry.kind, id]));
                continue;
            };
            match value
                .coerce(spec)
                .and_then(|coerced| params.set(id, coerced).map_err(PresetError::from))
            {
                Ok(_) => {}
                Err(error) => warnings.push(error.to_string()),
            }
        }
        let id = EffectSlot(self.next_slot);
        self.next_slot += 1;
        if let Some(engine) = &self.engine
            && engine.add_effect(id, effect).is_err()
        {
            return None;
        }
        self.slots.push(Slot {
            id,
            kind: descriptor.id,
            params,
            enabled: true,
        });
        Some(id)
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
