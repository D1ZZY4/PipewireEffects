use std::collections::BTreeMap;

use pipewire_effects_core::{ChainRecipe, EffectRecipe, ParamRepr};
use serde::{Deserialize, Serialize};

use crate::error::{PresetError, Result};

pub const SCHEMA_VERSION: u32 = 1;

const fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

const fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub output: ChainSection,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChainSection {
    #[serde(default)]
    pub bypassed: bool,
    #[serde(default)]
    pub effects: Vec<EffectEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectEntry {
    pub kind: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub parameters: BTreeMap<String, ParamRepr>,
}

impl Preset {
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            description: description.into(),
            author: String::new(),
            output: ChainSection::default(),
        }
    }

    #[must_use]
    pub fn from_recipe(
        name: impl Into<String>,
        description: impl Into<String>,
        recipe: &ChainRecipe,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            description: description.into(),
            author: String::new(),
            output: ChainSection {
                bypassed: recipe.bypassed,
                effects: recipe
                    .effects
                    .iter()
                    .map(|effect| EffectEntry {
                        kind: effect.kind.clone(),
                        enabled: effect.enabled,
                        parameters: effect.values.iter().cloned().collect(),
                    })
                    .collect(),
            },
        }
    }

    #[must_use]
    pub fn to_recipe(&self) -> ChainRecipe {
        ChainRecipe {
            bypassed: self.output.bypassed,
            effects: self
                .output
                .effects
                .iter()
                .map(|entry| EffectRecipe {
                    kind: entry.kind.clone(),
                    enabled: entry.enabled,
                    values: entry
                        .parameters
                        .iter()
                        .map(|(id, value)| (id.clone(), *value))
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version > SCHEMA_VERSION {
            return Err(PresetError::UnsupportedSchema {
                name: self.name.clone(),
                found: self.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(())
    }

    pub fn file_stem(&self) -> Result<String> {
        file_stem(&self.name)
    }

    pub fn effect_kinds(&self) -> impl Iterator<Item = &str> {
        self.output.effects.iter().map(|entry| entry.kind.as_str())
    }
}

pub fn file_stem(name: &str) -> Result<String> {
    let mut stem = String::with_capacity(name.len());
    let mut pending_dash = false;

    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_dash && !stem.is_empty() {
                stem.push('-');
            }
            pending_dash = false;
            stem.push(character.to_ascii_lowercase());
        } else if character.is_alphanumeric() {
            continue;
        } else {
            pending_dash = true;
        }
    }

    if stem.is_empty() {
        return Err(PresetError::InvalidName(name.to_owned()));
    }
    Ok(stem)
}

#[cfg(test)]
mod tests;
