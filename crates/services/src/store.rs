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

#[cfg(test)]
mod tests {
    use super::*;
    use pwe_core::PipelineKind;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("pwe-services-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn a_missing_file_loads_without_a_warning() {
        let store = SettingsStore::with_path(scratch("missing").join(FILE_NAME));
        let (settings, warning) = store.load_with_warning();
        assert_eq!(settings, Settings::default());
        assert!(warning.is_none());
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let store = SettingsStore::with_path(scratch("round-trip").join(FILE_NAME));
        let mut settings = Settings::default();
        settings.output.device = "usb".to_owned();
        settings.process_input = true;
        settings.autostart = true;
        store.save(&settings).expect("should save");
        assert_eq!(store.load(), settings);
    }

    #[test]
    fn a_corrupt_file_falls_back_to_the_defaults_with_a_warning() {
        let directory = scratch("corrupt");
        fs::create_dir_all(&directory).expect("should create");
        let path = directory.join(FILE_NAME);
        fs::write(&path, "{ not json").expect("should write");
        let store = SettingsStore::with_path(path);
        let (settings, warning) = store.load_with_warning();
        assert_eq!(settings, Settings::default());
        assert!(warning.is_some());
    }

    #[test]
    fn the_pipeline_selection_survives_a_round_trip() {
        let store = SettingsStore::with_path(scratch("pipeline").join(FILE_NAME));
        let mut settings = Settings::default();
        settings.pipeline_mut(PipelineKind::Input).device = "mic".to_owned();
        settings.audio_backend = "gpu".to_owned();
        store.save(&settings).expect("should save");
        assert_eq!(store.load().pipeline(PipelineKind::Input).device, "mic");
        assert_eq!(store.load().audio_backend, "gpu");
    }
}
