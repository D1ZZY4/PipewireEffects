use std::sync::{Arc, Mutex};

use crate::config::{AUTO_TARGET, EngineConfig};
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
    let explicit = !config.target.is_empty() && config.target != AUTO_TARGET;
    let wanted = if explicit {
        config.target.as_str()
    } else {
        system_default.unwrap_or_default()
    };
    if let Some(device) = device::resolve(devices, wanted, &config.sink_node_name) {
        return device.name.clone();
    }
    if explicit {
        shared.add_warning(format!("no output device matches '{}'", config.target));
    }
    device::resolve(devices, "", &config.sink_node_name)
        .map_or_else(String::new, |device| device.name.clone())
}
