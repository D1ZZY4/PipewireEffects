use std::fs;
use std::path::{Path, PathBuf};

use pwe_core::PipelineKind;
use pwe_core::i18n::tr;

use crate::builtin;
use crate::error::{PresetError, PresetResult};
use crate::model::{Pipeline, Preset, validate_name};

pub const EXTENSION: &str = "json";
pub const APPLICATION_DIR: &str = "pipewire-effects";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetInfo {
    pub name: String,
    pub builtin: bool,
    pub user_file: Option<PathBuf>,
    pub output: bool,
    pub input: bool,
}

impl PresetInfo {
    pub fn deletable(&self) -> bool {
        self.user_file.is_some()
    }

    pub fn covers(&self, kind: PipelineKind) -> bool {
        match kind {
            PipelineKind::Output => self.output,
            PipelineKind::Input => self.input,
        }
    }

    fn with_sections(mut self, preset: &Preset) -> Self {
        self.output |= !preset.output.is_empty();
        self.input |= !preset.input.is_empty();
        self
    }
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

    pub fn list(&self) -> (Vec<PresetInfo>, Vec<String>) {
        let mut infos: Vec<PresetInfo> = builtin::names()
            .into_iter()
            .map(|name| {
                let sections = builtin::find(name).and_then(Result::ok);
                let info = PresetInfo {
                    name: name.to_owned(),
                    builtin: true,
                    user_file: None,
                    output: false,
                    input: false,
                };
                sections.map_or(info.clone(), |(preset, _)| info.with_sections(&preset))
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
                    warnings.push(tr(
                        "{0} is unreadable: {1}",
                        &[&directory.display().to_string(), &error.to_string()],
                    ));
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
                if validate_name(&name).is_err() {
                    warnings.push(tr(
                        "{0} was skipped: '{1}' is not a valid preset name",
                        &[&path.display().to_string(), &name],
                    ));
                    continue;
                }
                match read_preset(&path) {
                    Ok((preset, conversion)) => {
                        if preset.name != name {
                            warnings.push(tr(
                                "{0} declares the name '{1}'",
                                &[&path.display().to_string(), &preset.name],
                            ));
                        }
                        for warning in conversion {
                            warnings.push(format!("{}: {warning}", path.display()));
                        }
                        if let Some(existing) = infos.iter_mut().find(|info| info.name == name) {
                            existing.builtin = false;
                            existing.output |= !preset.output.is_empty();
                            existing.input |= !preset.input.is_empty();
                            if is_user {
                                existing.user_file = Some(path);
                            }
                        } else {
                            infos.push(
                                PresetInfo {
                                    name,
                                    builtin: false,
                                    user_file: is_user.then_some(path),
                                    output: false,
                                    input: false,
                                }
                                .with_sections(&preset),
                            );
                        }
                    }
                    Err(error) => warnings.push(tr(
                        "{0} was skipped: {1}",
                        &[&path.display().to_string(), &error.to_string()],
                    )),
                }
            }
        }
        infos.sort_by_key(|info| info.name.to_lowercase());
        (infos, warnings)
    }

    pub fn load(&self, name: &str) -> PresetResult<Preset> {
        self.load_with_warnings(name).map(|(preset, _)| preset)
    }

    pub fn load_with_warnings(&self, name: &str) -> PresetResult<(Preset, Vec<String>)> {
        validate_name(name)?;
        let mut failure: Option<PresetError> = None;
        for directory in std::iter::once(&self.user_dir).chain(self.system_dirs.iter()) {
            let candidate = directory.join(format!("{name}.{EXTENSION}"));
            if !candidate.is_file() {
                continue;
            }
            match read_preset(&candidate) {
                Ok(loaded) => return Ok(loaded),
                Err(error) => failure = Some(error),
            }
        }
        if let Some(builtin) = builtin::find(name) {
            return builtin;
        }
        Err(failure.unwrap_or_else(|| PresetError::NotFound(name.to_owned())))
    }

    pub fn save(&self, preset: &Preset) -> PresetResult<PathBuf> {
        preset.validate()?;
        fs::create_dir_all(&self.user_dir)?;
        let path = self.user_dir.join(format!("{}.{EXTENSION}", preset.name));
        let text = serde_json::to_string_pretty(preset)?;
        write_atomically(&path, &text)?;
        Ok(path)
    }

    pub fn save_section(
        &self,
        name: &str,
        kind: PipelineKind,
        section: &Pipeline,
    ) -> PresetResult<PathBuf> {
        validate_name(name)?;
        let mut preset = match self.load(name) {
            Ok(existing) => existing,
            Err(PresetError::NotFound(_)) => Preset::new(name),
            Err(error) => return Err(error),
        };
        preset.name = name.to_owned();
        *preset.pipeline_mut(kind) = section.clone();
        self.save(&preset)
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

    pub fn import(
        &self,
        source: &Path,
        rename: Option<&str>,
    ) -> PresetResult<(Preset, Vec<String>)> {
        let (mut preset, warnings) = read_preset(source)?;
        if let Some(name) = rename {
            validate_name(name)?;
            preset.name = name.to_owned();
        }
        self.save(&preset)?;
        Ok((preset, warnings))
    }

    pub fn export(&self, name: &str, destination: &Path) -> PresetResult<()> {
        let preset = self.load(name)?;
        let text = serde_json::to_string_pretty(&preset)?;
        write_atomically(destination, &text)
    }

    pub fn duplicate(&self, source: &str, target: &str) -> PresetResult<PathBuf> {
        let mut preset = self.load(source)?;
        preset.name = target.to_owned();
        self.save(&preset)
    }
}

fn read_preset(path: &Path) -> PresetResult<(Preset, Vec<String>)> {
    let text = fs::read_to_string(path)?;
    let name = name_from_path(path).unwrap_or_else(|| "Imported".to_owned());
    builtin::parse(&text, &name)
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
mod tests;
