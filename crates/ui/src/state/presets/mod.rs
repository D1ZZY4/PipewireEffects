use std::path::PathBuf;

use pwe_core::i18n::t;
use pwe_core::{ALL_PIPELINES, PipelineKind};
use pwe_presets::{Pipeline, PresetError};

use super::State;
use crate::chain::Chain;

impl State {
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
            self.baselines[kind.index()] = Some(normalized(&section));
        }
        self.settings.preset = name.to_owned();
        self.persist();
        Ok(warnings)
    }

    pub fn load_preset_section(
        &mut self,
        kind: PipelineKind,
        name: &str,
    ) -> Result<Vec<String>, PresetError> {
        let (preset, mut warnings) = self.store.load_with_warnings(name)?;
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
        self.chain_mut(kind).apply(&section, &mut warnings);
        self.baselines[kind.index()] = Some(normalized(&section));
        self.settings.preset = name.to_owned();
        self.persist();
        Ok(warnings)
    }

    pub fn save_preset_section(
        &mut self,
        kind: PipelineKind,
        name: &str,
    ) -> Result<PathBuf, PresetError> {
        let mut section = Pipeline::default();
        self.chain(kind).snapshot(&mut section);
        let path = self.store.save_section(name, kind, &section)?;
        self.baselines[kind.index()] = Some(section);
        self.settings.preset = name.to_owned();
        self.persist();
        Ok(path)
    }

    pub fn preset_infos(&self, kind: PipelineKind) -> (Vec<pwe_presets::PresetInfo>, Vec<String>) {
        let (infos, warnings) = self.store.list();
        let covering = infos.into_iter().filter(|info| info.covers(kind)).collect();
        (covering, warnings)
    }

    pub fn delete_preset(&mut self, name: &str) -> Result<(), PresetError> {
        self.store.delete(name)?;
        if self.loaded_preset() == Some(name) {
            self.settings.preset.clear();
            self.persist();
        }
        Ok(())
    }

    pub fn unload_preset(&mut self, kind: PipelineKind) {
        self.chain_mut(kind).clear();
        self.baselines[kind.index()] = None;
        self.settings.preset.clear();
        self.persist();
    }

    pub fn is_modified(&self, kind: PipelineKind) -> bool {
        let Some(baseline) = &self.baselines[kind.index()] else {
            return false;
        };
        let mut current = Pipeline::default();
        self.chain(kind).snapshot(&mut current);
        &current != baseline
    }
}

fn normalized(section: &Pipeline) -> Pipeline {
    let mut chain = Chain::new(None);
    let mut warnings = Vec::new();
    chain.apply(section, &mut warnings);
    let mut snapshot = Pipeline::default();
    chain.snapshot(&mut snapshot);
    snapshot
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
