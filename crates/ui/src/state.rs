use std::path::PathBuf;

use pwe_core::i18n::{t, tr};
use pwe_core::{ALL_PIPELINES, PipelineKind};
use pwe_engine::{EngineConfig, EngineHandle, EngineResult, EngineState, Snapshot, StatsSnapshot};
use pwe_presets::{Preset, PresetError, PresetStore};
use pwe_settings::{Settings, SettingsStore};

use crate::chain::{Chain, Slot};

pub struct State {
    chains: [Chain; 2],
    active: PipelineKind,
    pub store: PresetStore,
    settings_store: SettingsStore,
    pub settings: Settings,
}

impl State {
    pub fn new() -> EngineResult<Self> {
        let settings_store = SettingsStore::from_environment();
        let (settings, warning) = settings_store.load_with_warning();
        let output = EngineHandle::spawn(EngineConfig::for_kind(PipelineKind::Output))?;
        let input = if settings.process_input {
            Some(EngineHandle::spawn(EngineConfig::for_kind(
                PipelineKind::Input,
            ))?)
        } else {
            None
        };
        let mut state = Self {
            chains: [Chain::new(Some(output)), Chain::new(input)],
            active: PipelineKind::Output,
            store: PresetStore::from_environment(),
            settings_store,
            settings,
        };
        state.apply_settings();
        if let Some(warning) = warning {
            state.warn(&warning);
        }
        Ok(state)
    }

    pub fn active(&self) -> PipelineKind {
        self.active
    }

    pub fn set_active(&mut self, kind: PipelineKind) {
        self.active = kind;
    }

    pub fn chain(&self, kind: PipelineKind) -> &Chain {
        &self.chains[kind.index()]
    }

    pub fn chain_mut(&mut self, kind: PipelineKind) -> &mut Chain {
        &mut self.chains[kind.index()]
    }

    pub fn active_chain(&self) -> &Chain {
        self.chain(self.active)
    }

    pub fn active_chain_mut(&mut self) -> &mut Chain {
        let active = self.active;
        self.chain_mut(active)
    }

    pub fn slots(&self) -> &[Slot] {
        &self.active_chain().slots
    }

    pub fn selected(&self) -> Option<pwe_engine::EffectSlot> {
        self.active_chain().selected
    }

    pub fn set_selected(&mut self, id: Option<pwe_engine::EffectSlot>) {
        self.active_chain_mut().selected = id;
    }

    pub fn loaded_preset(&self) -> Option<&str> {
        self.settings.preset()
    }

    pub fn bypassed(&self) -> bool {
        self.active_chain().bypassed
    }

    pub fn add(&mut self, kind: &str) -> Option<pwe_engine::EffectSlot> {
        self.active_chain_mut().add(kind)
    }

    pub fn remove(&mut self, id: pwe_engine::EffectSlot) {
        self.active_chain_mut().remove(id);
    }

    pub fn move_by(&mut self, id: pwe_engine::EffectSlot, delta: isize) {
        self.active_chain_mut().move_by(id, delta);
    }

    pub fn set_enabled(&mut self, id: pwe_engine::EffectSlot, enabled: bool) {
        self.active_chain_mut().set_enabled(id, enabled);
    }

    pub fn set_bypass(&mut self, bypassed: bool) {
        self.active_chain_mut().set_bypass(bypassed);
        self.settings.pipeline_mut(self.active).bypassed = bypassed;
        self.persist();
    }

    pub fn snapshot_of(&self, kind: PipelineKind) -> Snapshot {
        match self.chain(kind).engine() {
            Some(engine) => engine.shared().snapshot(),
            None => Snapshot {
                kind,
                ..Snapshot::default()
            },
        }
    }

    pub fn state_of(&self, kind: PipelineKind) -> EngineState {
        self.chain(kind)
            .engine()
            .map_or(EngineState::Stopped, |engine| engine.shared().state())
    }

    pub fn slot(&self, id: pwe_engine::EffectSlot) -> Option<&Slot> {
        self.active_chain().slot(id)
    }

    pub fn error_of(&self, kind: PipelineKind) -> Option<String> {
        self.chain(kind)
            .engine()
            .and_then(|engine| engine.shared().error())
    }

    pub fn stats_of(&self, kind: PipelineKind) -> StatsSnapshot {
        self.chain(kind)
            .engine()
            .map_or_else(StatsSnapshot::default, |engine| {
                engine.shared().stats().snapshot()
            })
    }

    pub fn awaiting_recycling(&self) -> usize {
        self.chain(self.active)
            .engine()
            .map_or(0, EngineHandle::awaiting_recycling)
    }

    pub fn set_target_device(&mut self, target: String) {
        let active = self.active;
        self.chain(active).set_target_device(target.clone());
        self.settings.pipeline_mut(active).device = target;
        self.persist();
    }

    pub fn set_input_enabled(&mut self, enabled: bool) -> EngineResult<()> {
        if self.settings.process_input == enabled {
            return Ok(());
        }
        self.settings.process_input = enabled;
        if enabled {
            let engine = EngineHandle::spawn(EngineConfig::for_kind(PipelineKind::Input))?;
            self.chain_mut(PipelineKind::Input).start(engine);
            self.apply_device(PipelineKind::Input);
        } else {
            self.chain_mut(PipelineKind::Input).stop();
            if self.active == PipelineKind::Input {
                self.active = PipelineKind::Output;
            }
        }
        self.persist();
        Ok(())
    }

    pub fn set_autostart(&mut self, enabled: bool) -> Result<(), String> {
        pwe_settings::set_enabled(
            enabled,
            &format!("{} --no-window", pwe_settings::DEFAULT_EXEC),
        )
        .map_err(|error| error.to_string())?;
        self.settings.autostart = enabled;
        self.persist();
        Ok(())
    }

    pub fn set_developer_mode(&mut self, enabled: bool) {
        if self.settings.developer_mode == enabled {
            return;
        }
        self.settings.developer_mode = enabled;
        self.persist();
    }

    pub fn restart(&mut self, kind: PipelineKind) -> EngineResult<()> {
        let chain = self.chain_mut(kind);
        if let Some(engine) = chain.engine_mut() {
            engine.restart()?;
            chain.reapply();
        }
        Ok(())
    }

    pub fn load_preset(&mut self, name: &str) -> Result<Vec<String>, PresetError> {
        let (preset, mut warnings) = self.store.load_with_warnings(name)?;
        for kind in ALL_PIPELINES {
            let section = preset.pipeline(kind).clone();
            if kind == PipelineKind::Input
                && !self.settings.process_input
                && !section.effects.is_empty()
            {
                warnings.push(
                    t("the preset carries an input chain; turn on microphone processing to run it")
                        .to_owned(),
                );
            }
            let chain = self.chain_mut(kind);
            chain.apply(&section, &mut warnings);
            chain.loaded_preset = Some(name.to_owned());
        }
        self.settings.preset = name.to_owned();
        self.persist();
        Ok(warnings)
    }

    pub fn snapshot_preset(&self, name: &str) -> Preset {
        let mut preset = Preset::new(name);
        self.chain(PipelineKind::Output)
            .snapshot(&mut preset.output);
        self.chain(PipelineKind::Input).snapshot(&mut preset.input);
        preset
    }

    pub fn save_preset(&mut self, name: &str) -> Result<PathBuf, PresetError> {
        let preset = self.snapshot_preset(name);
        let path = self.store.save(&preset)?;
        self.settings.preset = name.to_owned();
        self.persist();
        Ok(path)
    }

    pub fn delete_preset(&mut self, name: &str) -> Result<(), PresetError> {
        self.store.delete(name)?;
        if self.loaded_preset() == Some(name) {
            self.settings.preset.clear();
            self.persist();
        }
        Ok(())
    }

    pub fn settings_store(&self) -> &SettingsStore {
        &self.settings_store
    }

    pub fn persist(&self) {
        if let Err(error) = self.settings_store.save(&self.settings) {
            self.warn(&tr(
                "the settings could not be saved: {0}",
                &[&error.to_string()],
            ));
        }
    }

    pub fn warn(&self, message: &str) {
        if let Some(engine) = self.chains[PipelineKind::Output.index()].engine() {
            engine.shared().add_warning(message.to_owned());
        }
    }

    fn apply_settings(&mut self) {
        for kind in ALL_PIPELINES {
            self.apply_device(kind);
        }
        if let Some(name) = self.settings.preset().map(str::to_owned)
            && let Err(error) = self.load_preset(&name)
        {
            self.warn(&tr(
                "'{0}' could not be loaded: {1}",
                &[&name, &error.to_string()],
            ));
        }
    }

    fn apply_device(&mut self, kind: PipelineKind) {
        if self.settings.pipeline(kind).follows_the_system_default() {
            self.chain(kind).set_target_device(pwe_engine::AUTO_TARGET);
            return;
        }
        let device = self.settings.pipeline(kind).device.clone();
        self.chain(kind).set_target_device(device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwe_presets::PresetValue;

    #[test]
    fn a_blank_preset_snapshots_both_pipelines() {
        let preset = Preset::new("Chain");
        assert_eq!(preset.schema_version, pwe_presets::SCHEMA_VERSION);
        assert!(preset.output.effects.is_empty());
        assert!(preset.input.effects.is_empty());
    }

    #[test]
    fn only_changed_parameters_are_written() {
        let params = pwe_core::ParamSet::new(pwe_dsp::effects::gain_specs());
        let default = params.get("gain").unwrap();
        params.set_f32("gain", -3.0).unwrap();
        assert_ne!(params.get("gain").unwrap(), default);
        let entry = PresetValue::from_param(params.get("gain").unwrap());
        assert_eq!(entry, PresetValue::Integer(-3));
    }
}
