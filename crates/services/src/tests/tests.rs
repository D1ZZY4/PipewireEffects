use std::fs;
use std::path::PathBuf;

use pwe_core::PipelineKind;

use crate::autostart::{APPLICATION_ID, AUTOSTART_DIR, autostart_path, entry};
use crate::model::{AUTO, DEFAULT_AUTO_SAVE_INTERVAL, DEFAULT_SPECTRUM_RANGE, Settings};
use crate::store::{FILE_NAME, SettingsStore};

fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("pwe-services-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    directory
}

#[test]
fn a_missing_document_yields_the_documented_defaults() {
    let settings = Settings::default();
    assert!(settings.output.follows_the_system_default());
    assert!(settings.preset().is_none());
    assert!(!settings.process_input);
    assert!(!settings.autostart);
    assert!(!settings.developer_mode);
}

#[test]
fn partial_documents_keep_the_missing_fields() {
    let settings: Settings =
        serde_json::from_str("{\"output\":{\"device\":\"usb\"}}").expect("should parse");
    assert_eq!(settings.output.device, "usb");
    assert_eq!(settings.input.device, AUTO);
    assert!(!settings.autostart);
}

#[test]
fn a_document_without_the_auto_save_fields_keeps_the_defaults() {
    let settings: Settings =
        serde_json::from_str("{\"preset\":\"Default\"}").expect("should parse");
    assert!(!settings.auto_save);
    assert_eq!(settings.auto_save_interval, DEFAULT_AUTO_SAVE_INTERVAL);
    assert_eq!(settings.audio_backend, "both");
    assert_eq!(settings.spectrum_range, DEFAULT_SPECTRUM_RANGE);
}

#[test]
fn the_spectrum_range_round_trips() {
    let settings: Settings = serde_json::from_str("{\"spectrum_range\":2}").expect("should parse");
    assert_eq!(settings.spectrum_range, 2);
    let text = serde_json::to_string(&settings).expect("should write");
    let reloaded: Settings = serde_json::from_str(&text).expect("should parse again");
    assert_eq!(reloaded.spectrum_range, 2);
}

#[test]
fn the_autoload_rules_round_trip() {
    let settings: Settings =
        serde_json::from_str("{\"autoload\":{\"alsa_output.usb\":\"Podcast voice\"}}")
            .expect("should parse");
    assert_eq!(
        settings.autoload.get("alsa_output.usb").map(String::as_str),
        Some("Podcast voice")
    );
    let text = serde_json::to_string(&settings).expect("should write");
    let reloaded: Settings = serde_json::from_str(&text).expect("should parse again");
    assert_eq!(reloaded.autoload, settings.autoload);
}

#[test]
fn the_processing_backend_round_trips_through_the_document() {
    let settings: Settings =
        serde_json::from_str("{\"audio_backend\":\"both\"}").expect("should parse");
    assert_eq!(settings.audio_backend, "both");
    let text = serde_json::to_string(&settings).expect("should write");
    let reloaded: Settings = serde_json::from_str(&text).expect("should parse again");
    assert_eq!(reloaded.audio_backend, "both");
}

#[test]
fn the_kind_selects_the_pipeline() {
    let mut settings = Settings::default();
    settings.pipeline_mut(PipelineKind::Input).bypassed = true;
    assert!(settings.pipeline(PipelineKind::Input).bypassed);
    assert!(!settings.pipeline(PipelineKind::Output).bypassed);
}

#[test]
fn the_entry_names_the_application_and_its_icon() {
    let entry = entry("pipewire-effects --no-window");
    assert!(entry.contains("Type=Application"));
    assert!(entry.contains("Exec=pipewire-effects --no-window"));
    assert!(entry.contains(&format!("Icon={APPLICATION_ID}")));
    assert!(entry.ends_with('\n'));
}

#[test]
fn the_path_sits_below_the_autostart_directory() {
    let path = autostart_path();
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("io.github.D1ZZY4.PipewireEffects.desktop")
    );
    assert!(
        path.parent()
            .is_some_and(|parent| parent.ends_with(AUTOSTART_DIR))
    );
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
