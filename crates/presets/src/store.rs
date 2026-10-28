use std::fs;
use std::path::{Path, PathBuf};

use crate::builtin;
use crate::error::{PresetError, PresetResult};
use crate::model::{Preset, validate_name};

pub const EXTENSION: &str = "json";
pub const APPLICATION_DIR: &str = "pipewire-effects";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetInfo {
    pub name: String,
    pub builtin: bool,
    pub user_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PresetStore {
    user_dir: PathBuf,
    system_dirs: Vec<PathBuf>,
}

impl Default for PresetStore {
    fn default() -> Self {
        Self::from_environment()
    }
}

impl PresetStore {
    pub fn from_environment() -> Self {
        let user_dir = config_home().join(APPLICATION_DIR).join("presets");
        let mut system_dirs = Vec::new();
        if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
            for entry in dirs.split(':').filter(|entry| !entry.is_empty()) {
                system_dirs.push(PathBuf::from(entry).join(APPLICATION_DIR).join("presets"));
            }
        } else {
            system_dirs.push(PathBuf::from("/usr/local/share/pipewire-effects/presets"));
            system_dirs.push(PathBuf::from("/usr/share/pipewire-effects/presets"));
        }
        Self {
            user_dir,
            system_dirs,
        }
    }

    pub fn with_dirs(user_dir: PathBuf, system_dirs: Vec<PathBuf>) -> Self {
        Self {
            user_dir,
            system_dirs,
        }
    }

    pub fn user_dir(&self) -> &Path {
        &self.user_dir
    }

    pub fn list(&self) -> (Vec<PresetInfo>, Vec<String>) {
        let mut infos: Vec<PresetInfo> = builtin::names()
            .into_iter()
            .map(|name| PresetInfo {
                name: name.to_owned(),
                builtin: true,
                user_file: None,
            })
            .collect();
        let mut warnings = Vec::new();

        for directory in self
            .system_dirs
            .iter()
            .chain(std::iter::once(&self.user_dir))
        {
            let is_user = directory == &self.user_dir;
            let entries = match fs::read_dir(directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    warnings.push(format!("{} is unreadable: {error}", directory.display()));
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some(EXTENSION) {
                    continue;
                }
                let Some(name) = name_from_path(&path) else {
                    continue;
                };
                match read_preset(&path) {
                    Ok(preset) => {
                        if preset.name != name {
                            warnings.push(format!(
                                "{} declares the name '{}'",
                                path.display(),
                                preset.name
                            ));
                        }
                        if let Some(existing) = infos.iter_mut().find(|info| info.name == name) {
                            if is_user {
                                existing.builtin = false;
                                existing.user_file = Some(path);
                            }
                        } else {
                            infos.push(PresetInfo {
                                name,
                                builtin: false,
                                user_file: is_user.then_some(path),
                            });
                        }
                    }
                    Err(error) => warnings.push(format!("{} was skipped: {error}", path.display())),
                }
            }
        }
        infos.sort_by_key(|info| info.name.to_lowercase());
        (infos, warnings)
    }

    pub fn load(&self, name: &str) -> PresetResult<Preset> {
        validate_name(name)?;
        let file = self.user_dir.join(format!("{name}.{EXTENSION}"));
        if file.is_file() {
            return read_preset(&file);
        }
        for directory in &self.system_dirs {
            let candidate = directory.join(format!("{name}.{EXTENSION}"));
            if candidate.is_file() {
                return read_preset(&candidate);
            }
        }
        builtin::find(name).unwrap_or_else(|| Err(PresetError::NotFound(name.to_owned())))
    }

    pub fn save(&self, preset: &Preset) -> PresetResult<PathBuf> {
        preset.validate()?;
        fs::create_dir_all(&self.user_dir)?;
        let path = self.user_dir.join(format!("{}.{EXTENSION}", preset.name));
        let text = serde_json::to_string_pretty(preset)?;
        write_atomically(&path, &text)?;
        Ok(path)
    }

    pub fn delete(&self, name: &str) -> PresetResult<()> {
        validate_name(name)?;
        let path = self.user_dir.join(format!("{name}.{EXTENSION}"));
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(PresetError::NotFound(name.to_owned()))
            }
            Err(error) => Err(PresetError::Io(error)),
        }
    }

    pub fn import(&self, source: &Path, rename: Option<&str>) -> PresetResult<Preset> {
        let mut preset = read_preset(source)?;
        if let Some(name) = rename {
            validate_name(name)?;
            preset.name = name.to_owned();
        }
        self.save(&preset)?;
        Ok(preset)
    }

    pub fn export(&self, name: &str, destination: &Path) -> PresetResult<()> {
        let preset = self.load(name)?;
        let text = serde_json::to_string_pretty(&preset)?;
        write_atomically(destination, &text)
    }
}

fn read_preset(path: &Path) -> PresetResult<Preset> {
    let text = fs::read_to_string(path)?;
    let preset: Preset = serde_json::from_str(&text)?;
    preset.validate()?;
    Ok(preset)
}

fn write_atomically(path: &Path, text: &str) -> PresetResult<()> {
    let temporary = path.with_extension(EXTENSION.to_owned() + ".tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn name_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
}

fn config_home() -> PathBuf {
    if let Ok(value) = std::env::var("XDG_CONFIG_HOME")
        && !value.is_empty()
    {
        return PathBuf::from(value);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    PathBuf::from(home).join(".config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Preset;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pwe-presets-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saving_and_loading_round_trips() {
        let dir = scratch("roundtrip");
        let store = PresetStore::with_dirs(dir.clone(), Vec::new());
        let mut preset = Preset::new("My chain");
        preset.output.bypassed = true;
        store.save(&preset).unwrap();
        let loaded = store.load("My chain").unwrap();
        assert_eq!(loaded, preset);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn builtins_are_available_without_any_files() {
        let store = PresetStore::with_dirs(scratch("builtin"), Vec::new());
        let (infos, warnings) = store.list();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(infos.iter().any(|info| info.name == "Flat" && info.builtin));
        assert!(store.load("Flat").is_ok());
    }

    #[test]
    fn a_user_preset_shadows_a_builtin_and_stays_deletable() {
        let dir = scratch("shadow");
        let store = PresetStore::with_dirs(dir.clone(), Vec::new());
        let mut preset = Preset::new("Flat");
        preset.output.bypassed = true;
        store.save(&preset).unwrap();
        let (infos, _) = store.list();
        let entry = infos.iter().find(|info| info.name == "Flat").unwrap();
        assert!(!entry.builtin);
        assert!(entry.user_file.is_some());
        store.delete("Flat").unwrap();
        assert!(store.load("Flat").unwrap().output.effects.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn malformed_files_are_skipped_with_a_warning() {
        let dir = scratch("broken");
        fs::write(dir.join("broken.json"), "{ not json").unwrap();
        let store = PresetStore::with_dirs(dir.clone(), Vec::new());
        let (infos, warnings) = store.list();
        assert!(warnings.len() == 1, "{warnings:?}");
        assert!(!infos.iter().any(|info| info.name == "broken"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn loading_a_missing_preset_is_reported() {
        let store = PresetStore::with_dirs(scratch("missing"), Vec::new());
        assert!(matches!(
            store.load("Absent"),
            Err(PresetError::NotFound(_))
        ));
    }

    #[test]
    fn path_traversal_is_refused() {
        let store = PresetStore::with_dirs(scratch("traversal"), Vec::new());
        assert!(store.load("../etc/passwd").is_err());
        assert!(store.save(&Preset::new("../escape")).is_err());
        assert!(store.delete("a/b").is_err());
    }
}
