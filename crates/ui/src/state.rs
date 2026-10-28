use std::collections::BTreeMap;
use std::path::PathBuf;

use pwe_core::ParamSet;
use pwe_engine::{EffectSlot, EngineConfig, EngineHandle, EngineResult};
use pwe_presets::{EffectEntry, Preset, PresetError, PresetStore, PresetValue};

pub struct Slot {
    pub id: EffectSlot,
    pub kind: &'static str,
    pub params: ParamSet,
    pub enabled: bool,
}

pub struct State {
    pub engine: EngineHandle,
    pub slots: Vec<Slot>,
    pub selected: Option<EffectSlot>,
    pub store: PresetStore,
    pub loaded_preset: Option<String>,
    pub bypassed: bool,
    next_slot: u64,
}

impl State {
    pub fn new() -> EngineResult<Self> {
        let engine = EngineHandle::spawn(EngineConfig::default())?;
        Ok(Self {
            engine,
            slots: Vec::new(),
            selected: None,
            store: PresetStore::from_environment(),
            loaded_preset: None,
            bypassed: false,
            next_slot: 1,
        })
    }

    pub fn add(&mut self, kind: &str) -> Option<EffectSlot> {
        if self.slots.len() >= self.engine.max_effects() {
            return None;
        }
        let descriptor = pwe_dsp::find(kind)?;
        let (params, effect) = pwe_dsp::instantiate(kind)?;
        let id = EffectSlot(self.next_slot);
        self.next_slot += 1;
        self.engine.add_effect(id, effect).ok()?;
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
            let _ = self.engine.remove_effect(slot.id);
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
        let _ = self.engine.move_effect(id, target);
    }

    pub fn set_enabled(&mut self, id: EffectSlot, enabled: bool) {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) {
            slot.enabled = enabled;
            let _ = self.engine.set_effect_enabled(id, enabled);
        }
    }

    pub fn set_bypass(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
        let _ = self.engine.set_bypass(bypassed);
    }

    pub fn slot(&self, id: EffectSlot) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.id == id)
    }

    pub fn load_preset(&mut self, name: &str) -> Result<Vec<String>, PresetError> {
        let preset = self.store.load(name)?;
        let mut warnings = Vec::new();
        for slot in std::mem::take(&mut self.slots) {
            let _ = self.engine.remove_effect(slot.id);
        }
        self.selected = None;
        for entry in &preset.output.effects {
            match self.add_entry(entry, &mut warnings) {
                Some(id) => self.selected = Some(id),
                None => warnings.push(format!("'{}' is not available", entry.kind)),
            }
        }
        self.set_bypass(preset.output.bypassed);
        self.loaded_preset = Some(name.to_owned());
        Ok(warnings)
    }

    fn add_entry(&mut self, entry: &EffectEntry, warnings: &mut Vec<String>) -> Option<EffectSlot> {
        let descriptor = pwe_dsp::find(&entry.kind)?;
        let (params, effect) = pwe_dsp::instantiate(&entry.kind)?;
        for (id, value) in &entry.params {
            let Some(spec) = params.spec(id) else {
                warnings.push(format!("'{}' has no parameter '{id}'", entry.kind));
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
        let slot = EffectSlot(self.next_slot);
        self.next_slot += 1;
        self.engine.add_effect(slot, effect).ok()?;
        self.slots.push(Slot {
            id: slot,
            kind: descriptor.id,
            params,
            enabled: true,
        });
        Some(slot)
    }

    pub fn snapshot_preset(&self, name: &str) -> Preset {
        let mut preset = Preset::new(name);
        preset.output.bypassed = self.bypassed;
        for slot in &self.slots {
            let mut params = BTreeMap::new();
            for (id, value) in slot.params.entries() {
                if let Some(spec) = slot.params.spec(id)
                    && spec.default_value() != value
                {
                    params.insert(id.to_owned(), PresetValue::from_param(value));
                }
            }
            preset.output.effects.push(EffectEntry {
                kind: slot.kind.to_owned(),
                params,
            });
        }
        preset
    }

    pub fn save_preset(&mut self, name: &str) -> Result<PathBuf, PresetError> {
        let preset = self.snapshot_preset(name);
        let path = self.store.save(&preset)?;
        self.loaded_preset = Some(name.to_owned());
        Ok(path)
    }

    pub fn delete_preset(&mut self, name: &str) -> Result<(), PresetError> {
        let result = self.store.delete(name);
        if result.is_ok() && self.loaded_preset.as_deref() == Some(name) {
            self.loaded_preset = None;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preset_snapshot_round_trips_through_the_model() {
        let preset = Preset::new("Chain");
        assert_eq!(preset.schema_version, pwe_presets::SCHEMA_VERSION);
        assert!(preset.output.effects.is_empty());
    }

    #[test]
    fn only_changed_parameters_are_written() {
        let params = ParamSet::new(pwe_dsp::effects::gain_specs());
        let default = params.get("gain").unwrap();
        params.set_f32("gain", -3.0).unwrap();
        assert_ne!(params.get("gain").unwrap(), default);
        let entry = PresetValue::from_param(params.get("gain").unwrap());
        assert_eq!(entry, PresetValue::Integer(-3));
    }
}
