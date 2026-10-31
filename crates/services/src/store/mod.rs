#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::fs;
use std::path::PathBuf;

use pwe_core::i18n::tr;

use crate::error::SettingsResult;
use crate::model::Settings;

pub const FILE_NAME: &str = "settings.json";
pub const APPLICATION_DIR: &str = "pipewire-effects";

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::from_environment()
    }
}

impl SettingsStore {
    pub fn from_environment() -> Self {
        Self::with_path(config_home().join(APPLICATION_DIR).join(FILE_NAME))
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Settings {
        self.load_with_warning().0
    }

    pub fn load_with_warning(&self) -> (Settings, Option<String>) {
        match fs::read_to_string(&self.path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(settings) => (settings, None),
                Err(error) => (
                    Settings::default(),
                    Some(tr(
                        "{0} was ignored: {1}",
                        &[&self.path.display().to_string(), &error.to_string()],
                    )),
                ),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (Settings::default(), None)
            }
            Err(error) => (
                Settings::default(),
                Some(tr(
                    "{0} was ignored: {1}",
                    &[&self.path.display().to_string(), &error.to_string()],
                )),
            ),
        }
    }

    pub fn save(&self, settings: &Settings) -> SettingsResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(settings)?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, &self.path)?;
        Ok(())
    }
}

pub fn config_home() -> PathBuf {
    if let Ok(value) = std::env::var("XDG_CONFIG_HOME")
        && !value.is_empty()
    {
        return PathBuf::from(value);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    PathBuf::from(home).join(".config")
}
