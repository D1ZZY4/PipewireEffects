use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{PresetError, Result};
use crate::model::{Preset, file_stem};

const BUILTIN_SOURCES: &[&str] = &[
    include_str!("../../../data/presets/flat.json"),
    include_str!("../../../data/presets/bass-boost.json"),
    include_str!("../../../data/presets/vocal-clarity.json"),
    include_str!("../../../data/presets/podcast.json"),
    include_str!("../../../data/presets/night-mode.json"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetInfo {
    pub name: String,
    pub description: String,
    pub builtin: bool,
    pub path: Option<PathBuf>,
}

impl PresetInfo {
    #[must_use]
    pub const fn is_deletable(&self) -> bool {
        !self.builtin
    }
}

#[derive(Debug, Clone)]
pub struct PresetStore {
    user_dir: PathBuf,
}

impl PresetStore {
    pub fn discover() -> Result<Self> {
        let project = directories::ProjectDirs::from("io.github", "D1ZZY4", "pipewire-effects")
            .ok_or(PresetError::NoDataDirectory)?;
        Ok(Self {
            user_dir: project.data_dir().join("presets"),
        })
    }

    #[must_use]
    pub fn with_user_dir(user_dir: PathBuf) -> Self {
        Self { user_dir }
    }

    #[must_use]
    pub fn user_dir(&self) -> &Path {
        &self.user_dir
    }

    pub fn builtin() -> Result<Vec<Preset>> {
        BUILTIN_SOURCES
            .iter()
            .map(|source| {
                serde_json::from_str::<Preset>(source).map_err(|source_err| PresetError::Parse {
                    path: PathBuf::from("<built-in>"),
                    source: source_err,
                })
            })
            .collect()
    }

    #[must_use]
    pub fn list(&self) -> Vec<PresetInfo> {
        let mut infos: Vec<PresetInfo> = Self::builtin()
            .unwrap_or_default()
            .into_iter()
            .map(|preset| PresetInfo {
                name: preset.name,
                description: preset.description,
                builtin: true,
                path: None,
            })
            .collect();

        for (path, preset) in self.user_presets() {
            if let Some(existing) = infos.iter_mut().find(|info| info.name == preset.name) {
                existing.builtin = false;
                existing.description = preset.description;
                existing.path = Some(path);
                continue;
            }
            infos.push(PresetInfo {
                name: preset.name,
                description: preset.description,
                builtin: false,
                path: Some(path),
            });
        }

        infos.sort_by_key(|info| info.name.to_lowercase());
        infos
    }

    pub fn load(&self, name: &str) -> Result<Preset> {
        let stem = file_stem(name)?;
        let path = self.user_dir.join(format!("{stem}.json"));
        if path.is_file() {
            return read_preset(&path);
        }

        if let Some(preset) = Self::builtin()?
            .into_iter()
            .find(|preset| preset.name == name || preset.file_stem().is_ok_and(|s| s == stem))
        {
            return Ok(preset);
        }

        Err(PresetError::NotFound(name.to_owned()))
    }

    pub fn save(&self, preset: &Preset) -> Result<PathBuf> {
        let stem = preset.file_stem()?;
        fs::create_dir_all(&self.user_dir).map_err(|source| PresetError::Io {
            action: "create",
            path: self.user_dir.clone(),
            source,
        })?;

        let path = self.user_dir.join(format!("{stem}.json"));
        let json = serde_json::to_string_pretty(preset).map_err(|source| PresetError::Parse {
            path: path.clone(),
            source,
        })?;

        fs::write(&path, format!("{json}\n")).map_err(|source| PresetError::Io {
            action: "write",
            path: path.clone(),
            source,
        })?;

        Ok(path)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let stem = file_stem(name)?;
        let path = self.user_dir.join(format!("{stem}.json"));
        if !path.is_file() {
            return Err(PresetError::NotFound(name.to_owned()));
        }
        fs::remove_file(&path).map_err(|source| PresetError::Io {
            action: "remove",
            path,
            source,
        })
    }

    fn user_presets(&self) -> Vec<(PathBuf, Preset)> {
        let Ok(entries) = fs::read_dir(&self.user_dir) else {
            return Vec::new();
        };

        let mut presets = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                if let Ok(preset) = read_preset(&path) {
                    presets.push((path, preset));
                }
            }
        }
        presets.sort_by(|left, right| left.0.cmp(&right.0));
        presets
    }
}

fn read_preset(path: &Path) -> Result<Preset> {
    let text = fs::read_to_string(path).map_err(|source| PresetError::Io {
        action: "read",
        path: path.to_path_buf(),
        source,
    })?;
    let preset: Preset = serde_json::from_str(&text).map_err(|source| PresetError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    preset.validate()?;
    Ok(preset)
}

#[cfg(test)]
mod tests;
