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
#[path = "tests/tests.rs"]
mod tests;
