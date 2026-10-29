use std::rc::Rc;
use std::sync::{Arc, Mutex};

use pipewire as pw;
use pw::registry::Listener as RegistryListener;

use crate::config::EngineConfig;
use crate::defaults::{self, Watch};
use crate::device::DeviceInfo;
use crate::shared::Shared;

pub fn install(
    registry: &pw::registry::RegistryRc,
    config: &EngineConfig,
    shared: &Arc<Shared>,
    devices: &Arc<Mutex<Vec<DeviceInfo>>>,
    system_default: &Rc<Watch>,
) -> RegistryListener {
    let collected = Arc::clone(devices);
    let shared_for_listener = Arc::clone(shared);
    let removed = Arc::clone(devices);
    let shared_for_removal = Arc::clone(shared);
    let watched = Rc::clone(system_default);
    let registry_for_metadata = registry.clone();
    let kind = config.kind;

    registry
        .add_listener_local()
        .global(move |object| {
            if defaults::want_metadata(object) {
                watched.bind(&registry_for_metadata, object, kind);
                return;
            }
            if object.type_ != pw::types::ObjectType::Node {
                return;
            }
            let Some(props) = object.props else {
                return;
            };
            let Some(info) = DeviceInfo::from_properties(object.id, props) else {
                return;
            };
            if let Ok(mut known) = collected.lock()
                && !known.iter().any(|existing| existing.id == info.id)
            {
                known.push(info);
                shared_for_listener.update_snapshot(|snapshot| snapshot.devices = known.clone());
            }
        })
        .global_remove(move |id| {
            if let Ok(mut known) = removed.lock() {
                let before = known.len();
                known.retain(|device| device.id != id);
                if known.len() != before {
                    shared_for_removal.update_snapshot(|snapshot| snapshot.devices = known.clone());
                }
            }
        })
        .register()
}
