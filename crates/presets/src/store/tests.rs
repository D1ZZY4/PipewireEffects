use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

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
