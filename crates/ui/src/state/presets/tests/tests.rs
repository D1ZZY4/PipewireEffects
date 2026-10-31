use super::*;

#[test]
fn a_loaded_builtin_preset_normalises_to_a_stable_snapshot() {
    let store = pwe_presets::PresetStore::with_dirs("/nonexistent".into(), Vec::new());
    let (preset, warnings) = store
        .load_with_warnings("Default")
        .expect("the built-in preset should load");
    assert!(warnings.is_empty(), "{warnings:?}");
    let baseline = normalized(&preset.output);
    assert!(!baseline.effects.is_empty());
    assert_eq!(normalized(&baseline), baseline);
}
