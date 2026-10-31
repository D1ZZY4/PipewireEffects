use pwe_presets::{Preset, PresetValue};
#[test]
fn a_blank_preset_snapshots_both_pipelines() {
    let preset = Preset::new("Chain");
    assert_eq!(preset.schema_version, pwe_presets::SCHEMA_VERSION);
    assert!(preset.output.effects.is_empty());
    assert!(preset.input.effects.is_empty());
}

#[test]
fn only_changed_parameters_are_written() {
    let params = pwe_core::ParamSet::new(pwe_dsp::effects::gain_specs());
    let default = params.get("gain").unwrap();
    params.set_f32("gain", -3.0).unwrap();
    assert_ne!(params.get("gain").unwrap(), default);
    let entry = PresetValue::from_param(params.get("gain").unwrap());
    assert_eq!(entry, PresetValue::Integer(-3));
}
