//! The on-disk preset format.
//!
//! The format is deliberately boring JSON with a `schema_version` field, so
//! that presets stay diffable, hand-editable and shareable. It mirrors
//! [`ChainRecipe`] closely but is a separate type on purpose: the file format
//! is a compatibility surface that must not change just because an internal
//! type is refactored.
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "name": "Bass Boost",
//!   "description": "A gentle low-end lift, kept in check by a limiter.",
//!   "output": {
//!     "effects": [
//!       { "kind": "parametric-eq", "parameters": { "band1_gain": 6.0 } },
//!       { "kind": "limiter", "parameters": { "threshold": -1.0 } }
//!     ]
//!   }
//! }
//! ```

use std::collections::BTreeMap;

use pipewire_effects_core::{ChainRecipe, EffectRecipe, ParamRepr};
use serde::{Deserialize, Serialize};

use crate::error::{PresetError, Result};

/// The newest schema version this build can read.
pub const SCHEMA_VERSION: u32 = 1;

const fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

const fn enabled_by_default() -> bool {
    true
}

/// A stored chain of effects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    /// File format version. Present so the format can evolve without breaking
    /// older files or older builds.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Display name. Also the key presets are looked up by.
    pub name: String,
    /// One-line explanation shown in the preset browser.
    #[serde(default)]
    pub description: String,
    /// Who wrote it. Free-form.
    #[serde(default)]
    pub author: String,
    /// The chain applied to the output (playback) pipeline.
    #[serde(default)]
    pub output: ChainSection,
}

/// One direction of the pipeline.
///
/// Only the output pipeline exists today. The struct is separate so that an
/// `input` section can be added under `schema_version: 2` without disturbing
/// anything that already works.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChainSection {
    /// Whether the whole chain starts bypassed.
    #[serde(default)]
    pub bypassed: bool,
    /// The effects, in processing order.
    #[serde(default)]
    pub effects: Vec<EffectEntry>,
}

/// One effect in a preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectEntry {
    /// The registered effect kind, for example `parametric-eq`.
    pub kind: String,
    /// Whether the effect starts enabled.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// Parameter values keyed by identifier.
    ///
    /// A `BTreeMap` rather than a `HashMap` so that saving a preset produces a
    /// stable, alphabetically ordered file that diffs cleanly. Sorted output is
    /// also easier to review by hand.
    #[serde(default)]
    pub parameters: BTreeMap<String, ParamRepr>,
}

impl Preset {
    /// Creates an empty preset.
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

    /// Captures a chain recipe as a preset.
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

    /// Converts the preset into a chain recipe the domain can apply.
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

    /// Checks that this build understands the file.
    ///
    /// # Errors
    ///
    /// Returns [`PresetError::UnsupportedSchema`] when the file was written by
    /// a newer version of the application.
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

    /// The file name this preset should be stored under, without extension.
    ///
    /// Names are forced to lowercase ASCII with single dashes, so that a preset
    /// called "Vocal Clarity!" and one called "vocal clarity" do not end up as
    /// two confusingly similar files.
    ///
    /// # Errors
    ///
    /// Returns [`PresetError::InvalidName`] when the name has no usable
    /// characters at all.
    pub fn file_stem(&self) -> Result<String> {
        file_stem(&self.name)
    }

    /// The effect kinds this preset refers to.
    pub fn effect_kinds(&self) -> impl Iterator<Item = &str> {
        self.output.effects.iter().map(|entry| entry.kind.as_str())
    }
}

/// Normalises a preset name into a file stem.
///
/// # Errors
///
/// Returns [`PresetError::InvalidName`] when nothing usable remains.
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
            // Non-ASCII letters are dropped rather than transliterated; the
            // display name keeps them, only the file name is restricted.
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
mod tests {
    use super::*;

    #[test]
    fn file_stems_are_normalised() {
        assert_eq!(file_stem("Bass Boost").unwrap(), "bass-boost");
        assert_eq!(file_stem("  Vocal   Clarity!! ").unwrap(), "vocal-clarity");
        assert_eq!(file_stem("Night/Mode").unwrap(), "night-mode");
        assert_eq!(file_stem("Already-good").unwrap(), "already-good");
        assert!(file_stem("!!").is_err());
        assert!(file_stem("").is_err());
    }

    #[test]
    fn named_presets_round_trip_within_the_schema_version() {
        let preset = Preset::new("Test", "A test.");
        assert!(preset.validate().is_ok());

        let mut newer = preset;
        newer.schema_version = SCHEMA_VERSION + 1;
        assert!(matches!(
            newer.validate(),
            Err(PresetError::UnsupportedSchema { .. })
        ));
    }

    #[test]
    fn a_missing_schema_version_defaults_to_the_current_one() {
        let preset: Preset = serde_json::from_str(r#"{ "name": "Bare" }"#).unwrap();
        assert_eq!(preset.schema_version, SCHEMA_VERSION);
        assert!(preset.output.effects.is_empty());
        assert!(!preset.output.bypassed);
    }

    #[test]
    fn effects_are_enabled_unless_stated_otherwise() {
        let preset: Preset = serde_json::from_str(
            r#"{ "name": "Bare", "output": { "effects": [ { "kind": "gain" } ] } }"#,
        )
        .unwrap();
        assert!(preset.output.effects[0].enabled);
        assert!(preset.output.effects[0].parameters.is_empty());
    }

    #[test]
    fn parameter_values_keep_their_json_shape() {
        let preset: Preset = serde_json::from_str(
            r#"{ "name": "P", "output": { "effects": [ { "kind": "gain",
                 "parameters": { "gain": -3.5, "mute": true, "mode": 2 } } ] } }"#,
        )
        .unwrap();
        let parameters = &preset.output.effects[0].parameters;
        assert_eq!(parameters["gain"], ParamRepr::Number(-3.5));
        assert_eq!(parameters["mute"], ParamRepr::Bool(true));
        assert_eq!(parameters["mode"], ParamRepr::Integer(2));

        // And back out to JSON identically.
        let json = serde_json::to_string(&preset).unwrap();
        assert!(json.contains(r#""gain":-3.5"#));
        assert!(json.contains(r#""mute":true"#));
        assert!(json.contains(r#""mode":2"#));
    }

    #[test]
    fn recipes_survive_a_conversion_round_trip() {
        let recipe = ChainRecipe {
            bypassed: true,
            effects: vec![EffectRecipe {
                kind: "parametric-eq".to_owned(),
                enabled: false,
                values: vec![
                    ("band1_gain".to_owned(), ParamRepr::Number(6.0)),
                    ("band1_enabled".to_owned(), ParamRepr::Bool(true)),
                ],
            }],
        };
        let preset = Preset::from_recipe("Round Trip", "", &recipe);

        // Parameters are stored in a `BTreeMap` so that saved presets diff
        // cleanly, which means they come back sorted by identifier. The values
        // themselves are unchanged.
        let mut expected = recipe;
        expected.effects[0]
            .values
            .sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(preset.to_recipe(), expected);
    }
}
