#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use ash::vk;

pub struct DeviceInfo {
    pub name: String,
    pub kind: vk::PhysicalDeviceType,
}

pub fn devices() -> Vec<DeviceInfo> {
    let Ok(entry) = (unsafe { ash::Entry::load() }) else {
        return Vec::new();
    };
    let app = vk::ApplicationInfo::default().api_version(vk::make_api_version(0, 1, 1, 0));
    let create = vk::InstanceCreateInfo::default().application_info(&app);
    let Ok(instance) = (unsafe { entry.create_instance(&create, None) }) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    if let Ok(physical) = unsafe { instance.enumerate_physical_devices() } {
        for device in physical {
            let properties = unsafe { instance.get_physical_device_properties(device) };
            let name = properties
                .device_name_as_c_str()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default();
            found.push(DeviceInfo {
                name,
                kind: properties.device_type,
            });
        }
    }
    unsafe { instance.destroy_instance(None) };
    found
}
