//! Where presets live.
//!
//! Two sources, in priority order:
//!
//! * **Built-in presets** are compiled into the binary. They are the ones that
//!   make the application useful on first run, and they cannot be broken by a
//!   user edit.
//! * **User presets** live in the project's data directory, which follows the
//!   XDG base directory specification (`~/.local/share/pipewire-effects/presets`
//!   on a default Linux setup).
//!
//! Saving a preset whose name matches a built-in one shadows the built-in
//! rather than overwriting it, because built-ins are part of the binary and
//! cannot be overwritten anyway. Deleting such a preset restores the built-in.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{PresetError, Result};
use crate::model::{Preset, file_stem};

/// The presets compiled into the binary.
///
/// Parsing these is exercised by `tests/builtin_presets.rs`, which also checks
/// that every effect and parameter they mention actually exists.
const BUILTIN_SOURCES: &[&str] = &[
    include_str!("../../../data/presets/flat.json"),
    include_str!("../../../data/presets/bass-boost.json"),
    include_str!("../../../data/presets/vocal-clarity.json"),
    include_str!("../../../data/presets/podcast.json"),
    include_str!("../../../data/presets/night-mode.json"),
];

/// A summary of a preset, for the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetInfo {
    /// Display name.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Whether it ships with the application.
    pub builtin: bool,
    /// The file it came from, for user presets.
    pub path: Option<PathBuf>,
}

impl PresetInfo {
    /// Whether the preset can be deleted by the user.
    #[must_use]
    pub const fn is_deletable(&self) -> bool {
        !self.builtin
    }
}

/// Reads and writes presets.
#[derive(Debug, Clone)]
pub struct PresetStore {
    user_dir: PathBuf,
}

impl PresetStore {
    /// Finds the user preset directory for this platform.
    ///
    /// The directory is created on first save; [`PresetStore::list`] works
    /// whether or not it exists yet.
    ///
    /// # Errors
    ///
    /// Returns [`PresetError::NoDataDirectory`] when the platform has no
    /// notion of a per-user data directory.
    pub fn discover() -> Result<Self> {
        let project = directories::ProjectDirs::from("io.github", "D1ZZY4", "pipewire-effects")
            .ok_or(PresetError::NoDataDirectory)?;
        Ok(Self {
            user_dir: project.data_dir().join("presets"),
        })
    }

    /// Builds a store rooted at an explicit directory. Used by tests.
    #[must_use]
    pub fn with_user_dir(user_dir: PathBuf) -> Self {
        Self { user_dir }
    }

    /// The directory user presets are read from and written to.
    #[must_use]
    pub fn user_dir(&self) -> &Path {
        &self.user_dir
    }

    /// The presets compiled into the binary.
    ///
    /// # Errors
    ///
    /// Returns a parse error if a built-in preset is malformed, which would be
    /// a build-time mistake rather than a user-facing one.
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

    /// Summarises every available preset, sorted by name.
    ///
    /// A user preset with the same name as a built-in appears **once**, with
    /// the user's file as its source: `load` returns that file, and deleting it
    /// falls back to the built-in. Reporting it as a user preset is what makes
    /// that possible from the interface.
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

    /// Loads a preset by name, preferring the user directory.
    ///
    /// # Errors
    ///
    /// Returns [`PresetError::NotFound`] when no preset has that name, and the
    /// underlying I/O or parse error when the file exists but cannot be read.
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

    /// Writes a preset to the user directory, returning the file it wrote.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the directory cannot be created or the file
    /// cannot be written, and [`PresetError::InvalidName`] for a name with no
    /// usable characters.
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

    /// Removes a user preset.
    ///
    /// # Errors
    ///
    /// Returns [`PresetError::NotFound`] when there is no user preset with that
    /// name. Deleting a *built-in* preset is similarly refused, since there is
    /// no file to remove.
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

    /// Every readable preset in the user directory, with its path.
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
                // A malformed user preset must not stop the browser from
                // listing the rest; it simply does not appear.
                if let Ok(preset) = read_preset(&path) {
                    presets.push((path, preset));
                }
            }
        }
        presets.sort_by(|left, right| left.0.cmp(&right.0));
        presets
    }
}

/// Reads and validates a preset file.
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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A scratch directory that cleans itself up.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pipewire-effects-presets-{}-{unique}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn store() -> (Scratch, PresetStore) {
        let scratch = Scratch::new();
        let store = PresetStore::with_user_dir(scratch.path.clone());
        (scratch, store)
    }

    #[test]
    fn the_builtin_presets_parse() {
        let builtins = PresetStore::builtin().unwrap();
        assert_eq!(builtins.len(), BUILTIN_SOURCES.len());
        for preset in &builtins {
            assert!(!preset.name.is_empty(), "a built-in preset has no name");
            assert!(preset.file_stem().is_ok());
        }
    }

    #[test]
    fn an_empty_directory_still_lists_the_builtins() {
        let (_scratch, store) = store();
        let infos = store.list();
        assert_eq!(infos.len(), BUILTIN_SOURCES.len());
        assert!(infos.iter().all(|info| info.builtin));
        assert!(infos.iter().all(|info| !info.is_deletable()));
    }

    #[test]
    fn saving_then_loading_round_trips() {
        let (_scratch, store) = store();
        let mut preset = Preset::new("My Sound", "Something I made.");
        preset.author = "tester".to_owned();

        let path = store.save(&preset).unwrap();
        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "my-sound.json");

        let loaded = store.load("My Sound").unwrap();
        assert_eq!(loaded, preset);
    }

    #[test]
    fn user_presets_appear_alongside_the_builtins() {
        let (_scratch, store) = store();
        store.save(&Preset::new("Zebra", "")).unwrap();
        store.save(&Preset::new("Apple", "")).unwrap();

        let infos = store.list();
        assert_eq!(infos.len(), BUILTIN_SOURCES.len() + 2);
        assert!(
            infos
                .iter()
                .any(|info| info.name == "Zebra" && !info.builtin)
        );
        assert!(
            infos
                .iter()
                .any(|info| info.name == "Apple" && !info.builtin)
        );

        // Sorted case-insensitively.
        let names: Vec<&str> = infos.iter().map(|info| info.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_by_key(|name| name.to_lowercase());
        assert_eq!(names, sorted);
    }

    #[test]
    fn deleting_removes_a_user_preset() {
        let (_scratch, store) = store();
        store.save(&Preset::new("Temporary", "")).unwrap();
        assert!(store.load("Temporary").is_ok());

        store.delete("Temporary").unwrap();
        assert!(matches!(
            store.load("Temporary"),
            Err(PresetError::NotFound(_))
        ));
    }

    #[test]
    fn deleting_a_builtin_is_refused() {
        let (_scratch, store) = store();
        let builtin_name = PresetStore::builtin().unwrap()[0].name.clone();
        assert!(matches!(
            store.delete(&builtin_name),
            Err(PresetError::NotFound(_))
        ));
    }

    #[test]
    fn unknown_presets_report_not_found() {
        let (_scratch, store) = store();
        assert!(matches!(
            store.load("does not exist"),
            Err(PresetError::NotFound(_))
        ));
    }

    #[test]
    fn a_malformed_user_preset_is_skipped_rather_than_fatal() {
        let (scratch, store) = store();
        fs::create_dir_all(&scratch.path).unwrap();
        fs::write(scratch.path.join("broken.json"), "{ not json").unwrap();
        store.save(&Preset::new("Fine", "")).unwrap();

        let infos = store.list();
        assert!(infos.iter().any(|info| info.name == "Fine"));
        assert!(!infos.iter().any(|info| info.name == "broken"));
    }

    #[test]
    fn a_newer_schema_version_is_rejected_on_load() {
        let (scratch, store) = store();
        fs::create_dir_all(&scratch.path).unwrap();
        fs::write(
            scratch.path.join("future.json"),
            r#"{ "schema_version": 99, "name": "Future" }"#,
        )
        .unwrap();

        assert!(matches!(
            store.load("Future"),
            Err(PresetError::UnsupportedSchema { .. })
        ));
    }

    #[test]
    fn saved_files_are_pretty_printed_and_newline_terminated() {
        let (scratch, store) = store();
        let path = store.save(&Preset::new("Pretty", "Yes.")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.ends_with("}\n"));
        assert!(
            text.contains("\n  \"name\": \"Pretty\""),
            "not pretty: {text}"
        );
        let _ = scratch;
    }

    #[test]
    fn user_presets_shadow_builtins_without_duplicating_them() {
        let (_scratch, store) = store();
        let builtin = PresetStore::builtin().unwrap()[0].clone();
        let mut shadow = builtin.clone();
        shadow.description = "My own version of this.".to_owned();
        store.save(&shadow).unwrap();

        let infos = store.list();
        let matching: Vec<&PresetInfo> = infos
            .iter()
            .filter(|info| info.name == builtin.name)
            .collect();
        assert_eq!(matching.len(), 1, "the name should appear once");

        // The file wins, and because the file is behind the entry the user can
        // delete it to fall back to the built-in version.
        let loaded = store.load(&builtin.name).unwrap();
        assert_eq!(loaded.description, "My own version of this.");
        assert!(!matching[0].builtin);
        assert!(matching[0].is_deletable());

        store.delete(&builtin.name).unwrap();
        let restored = store.load(&builtin.name).unwrap();
        assert_eq!(
            restored, builtin,
            "deleting should fall back to the built-in"
        );
    }
}
