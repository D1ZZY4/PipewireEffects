use std::sync::{Arc, Mutex};

use pwe_core::i18n::tr;

use crate::config::EngineConfig;
use crate::device::{self, DeviceInfo};
use crate::shared::Shared;

pub fn report(shared: &Shared, devices: &Arc<Mutex<Vec<DeviceInfo>>>) {
    let known = devices
        .lock()
        .map_or_else(|_| Vec::new(), |known| known.clone());
    shared.update_snapshot(move |snapshot| snapshot.devices = known);
}

pub fn resolve(
    shared: &Shared,
    devices: &[DeviceInfo],
    config: &EngineConfig,
    system_default: Option<&str>,
) -> String {
    let own = config.own_nodes();
    let own = own.as_slice();
    let wanted = if config.follows_the_system_default() {
        system_default.unwrap_or_default()
    } else {
        config.target.as_str()
    };
    if let Some(device) = device::resolve(devices, wanted, own, config.kind) {
        return device.name.clone();
    }
    if !config.follows_the_system_default() {
        shared.add_warning(tr(
            "no {0} device matches '{1}'",
            &[config.kind.noun(), &config.target],
        ));
    }
    device::resolve(devices, "", own, config.kind)
        .map_or_else(String::new, |device| device.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{MEDIA_CLASS_SINK, MEDIA_CLASS_SOURCE};
    use pwe_core::PipelineKind;

    fn device(name: &str, class: &str, priority: i64) -> DeviceInfo {
        DeviceInfo {
            serial: 0,
            id: 1,
            name: name.to_owned(),
            description: name.to_owned(),
            media_class: class.to_owned(),
            virtual_device: false,
            priority,
        }
    }

    fn devices() -> Vec<DeviceInfo> {
        vec![
            device("speakers", MEDIA_CLASS_SINK, 600),
            device("usb", MEDIA_CLASS_SINK, 900),
            device("mic", MEDIA_CLASS_SOURCE, 600),
            device("webcam", MEDIA_CLASS_SOURCE, 900),
        ]
    }

    #[test]
    fn the_output_pipeline_follows_the_system_default_sink() {
        let shared = Shared::new();
        let config = EngineConfig::for_kind(PipelineKind::Output);
        assert_eq!(
            resolve(&shared, &devices(), &config, Some("speakers")),
            "speakers"
        );
    }

    #[test]
    fn the_input_pipeline_follows_the_system_default_source() {
        let shared = Shared::new();
        let config = EngineConfig::for_kind(PipelineKind::Input);
        assert_eq!(resolve(&shared, &devices(), &config, Some("mic")), "mic");
    }

    #[test]
    fn an_explicit_choice_wins_and_a_missing_one_is_reported() {
        let shared = Shared::new();
        let mut config = EngineConfig::for_kind(PipelineKind::Input);
        config.target = "webcam".to_owned();
        assert_eq!(resolve(&shared, &devices(), &config, None), "webcam");
        config.target = "absent".to_owned();
        assert_eq!(resolve(&shared, &devices(), &config, None), "webcam");
        assert!(
            shared
                .snapshot()
                .warnings
                .iter()
                .any(|warning| warning.contains("absent"))
        );
    }

    #[test]
    fn an_output_configuration_never_settles_on_a_source() {
        let shared = Shared::new();
        let config = EngineConfig::for_kind(PipelineKind::Output);
        assert_eq!(resolve(&shared, &devices(), &config, None), "usb");
    }
}
