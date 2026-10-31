use std::collections::BTreeMap;

use super::*;
use crate::model::{EffectEntry, Preset};

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
    assert!(
        infos
            .iter()
            .any(|info| info.name == "Default" && info.builtin)
    );
    assert!(store.load("Default").is_ok());
}

#[test]
fn a_user_preset_shadows_a_builtin_and_stays_deletable() {
    let dir = scratch("shadow");
    let store = PresetStore::with_dirs(dir.clone(), Vec::new());
    let mut preset = Preset::new("Default");
    preset.output.bypassed = true;
    store.save(&preset).unwrap();
    let (infos, _) = store.list();
    let entry = infos.iter().find(|info| info.name == "Default").unwrap();
    assert!(!entry.builtin);
    assert!(entry.user_file.is_some());
    store.delete("Default").unwrap();
    let restored = store.load("Default").unwrap();
    assert!(!restored.output.effects.is_empty());
    assert!(!restored.output.bypassed);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_system_file_shadows_a_builtin_in_the_list_and_in_the_loader() {
    let user = scratch("system-shadow-user");
    let system = scratch("system-shadow-system");
    fs::write(
        system.join("Default.json"),
        r#"{ "schema_version": 1, "name": "Default",
             "output": { "bypassed": true, "effects": [] } }"#,
    )
    .unwrap();
    let store = PresetStore::with_dirs(user.clone(), vec![system.clone()]);
    let (infos, _) = store.list();
    let entry = infos
        .iter()
        .find(|info| info.name == "Default")
        .expect("the default entry exists");
    assert!(!entry.builtin);
    assert!(!entry.deletable());
    assert!(store.load("Default").unwrap().output.bypassed);
    fs::remove_dir_all(&user).unwrap();
    fs::remove_dir_all(&system).unwrap();
}

#[test]
fn a_malformed_file_does_not_hide_the_built_in_it_shadows() {
    let dir = scratch("malformed-shadow");
    fs::write(dir.join("Default.json"), "{ not json").unwrap();
    let store = PresetStore::with_dirs(dir.clone(), Vec::new());
    let (infos, warnings) = store.list();
    let entry = infos
        .iter()
        .find(|info| info.name == "Default")
        .expect("the default entry exists");
    assert!(entry.builtin);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("Default.json")),
        "{warnings:?}"
    );
    let loaded = store
        .load("Default")
        .expect("the built-in is still loadable");
    assert!(!loaded.output.effects.is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_file_whose_name_is_not_a_valid_preset_is_not_listed() {
    let dir = scratch("invalid-name");
    fs::write(
        dir.join(".hidden.json"),
        r#"{ "schema_version": 1, "name": "hidden", "output": {} }"#,
    )
    .unwrap();
    let store = PresetStore::with_dirs(dir.clone(), Vec::new());
    let (infos, warnings) = store.list();
    assert!(!infos.iter().any(|info| info.name == ".hidden"));
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("not a valid preset name")),
        "{warnings:?}"
    );
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

#[test]
fn a_reference_preset_surfaces_its_conversion_warnings() {
    let dir = scratch("conversion");
    fs::write(
        dir.join("imported.json"),
        r#"{ "output": { "plugins_order": ["gate#0"],
             "gate#0": { "curve-threshold": -30.0, "makeup": 3.0 } } }"#,
    )
    .unwrap();
    let store = PresetStore::with_dirs(dir.clone(), Vec::new());
    let (preset, warnings) = store.load_with_warnings("imported").unwrap();
    assert_eq!(preset.output.effects[0].kind, "gate");
    assert!(
        warnings.iter().any(|warning| warning.contains("makeup")),
        "{warnings:?}"
    );
    let (_, listed) = store.list();
    assert!(
        listed.iter().any(|warning| warning.contains("makeup")),
        "{listed:?}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_import_reports_what_the_conversion_dropped() {
    let dir = scratch("import");
    let source = dir.join("reference.json");
    fs::write(
        &source,
        r#"{ "output": { "plugins_order": ["reverb#0"],
             "reverb#0": { "room-size": "Large", "decay-time": 0.4 } } }"#,
    )
    .unwrap();
    let store = PresetStore::with_dirs(dir.clone(), Vec::new());
    let (preset, warnings) = store.import(&source, Some("Imported reference")).unwrap();
    assert_eq!(preset.name, "Imported reference");
    assert!(
        warnings.iter().any(|warning| warning.contains("room-size")),
        "{warnings:?}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_preset_reports_which_pipelines_it_covers() {
    let dir = scratch("sections");
    let mut preset = Preset::new("Split");
    preset.output.effects.push(EffectEntry {
        kind: "gain".to_owned(),
        enabled: true,
        params: BTreeMap::new(),
    });
    store_with(&dir).save(&preset).unwrap();
    let (infos, _) = store_with(&dir).list();
    let entry = infos.iter().find(|info| info.name == "Split").unwrap();
    assert!(entry.covers(PipelineKind::Output));
    assert!(!entry.covers(PipelineKind::Input));

    let (builtins, _) = PresetStore::with_dirs(scratch("sections-builtin"), Vec::new()).list();
    let default = builtins.iter().find(|info| info.name == "Default").unwrap();
    assert!(default.covers(PipelineKind::Output));
    assert!(!default.covers(PipelineKind::Input));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saving_one_pipeline_keeps_the_other_section() {
    let dir = scratch("save-section");
    let store = store_with(&dir);
    let mut preset = Preset::new("Both");
    preset.input.bypassed = true;
    store.save(&preset).unwrap();

    let mut output = Pipeline::default();
    output.effects.push(EffectEntry {
        kind: "limiter".to_owned(),
        enabled: true,
        params: BTreeMap::new(),
    });
    store
        .save_section("Both", PipelineKind::Output, &output)
        .unwrap();

    let loaded = store.load("Both").unwrap();
    assert_eq!(loaded.output.effects.len(), 1);
    assert!(loaded.input.bypassed);
}

#[test]
fn saving_a_new_pipeline_section_creates_the_preset() {
    let dir = scratch("save-new-section");
    let store = store_with(&dir);
    let mut input = Pipeline::default();
    input.effects.push(EffectEntry {
        kind: "gate".to_owned(),
        enabled: false,
        params: BTreeMap::new(),
    });
    store
        .save_section("Mic", PipelineKind::Input, &input)
        .unwrap();
    let loaded = store.load("Mic").unwrap();
    assert_eq!(loaded.input.effects.len(), 1);
    assert!(loaded.output.effects.is_empty());
}

#[test]
fn duplicating_a_preset_writes_a_copy_under_the_new_name() {
    let dir = scratch("duplicate");
    let store = store_with(&dir);
    let path = store.duplicate("Default", "Default copy").unwrap();
    assert!(path.ends_with("Default copy.json"));
    let source = store.load("Default").unwrap();
    let copy = store.load("Default copy").unwrap();
    assert_eq!(copy.name, "Default copy");
    assert!(!copy.output.effects.is_empty());
    assert_eq!(copy.output.effects.len(), source.output.effects.len());
}

#[test]
fn duplicating_refuses_an_invalid_target_or_a_missing_source() {
    let dir = scratch("duplicate-invalid");
    let store = store_with(&dir);
    assert!(store.duplicate("Default", "../escape").is_err());
    assert!(store.duplicate("Absent", "Copy").is_err());
}

fn store_with(dir: &Path) -> PresetStore {
    PresetStore::with_dirs(dir.to_path_buf(), Vec::new())
}
