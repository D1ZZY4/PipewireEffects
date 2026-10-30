use ash::vk;

pub(super) fn pick(instance: &ash::Instance) -> Option<(vk::PhysicalDevice, u32)> {
    let devices = unsafe { instance.enumerate_physical_devices() }.ok()?;
    for device in devices {
        let families = unsafe { instance.get_physical_device_queue_family_properties(device) };
        if let Some(index) = families
            .iter()
            .position(|family| family.queue_flags.contains(vk::QueueFlags::COMPUTE))
        {
            return Some((device, index as u32));
        }
    }
    None
}

pub(super) fn binding(binding: u32) -> vk::DescriptorSetLayoutBinding<'static> {
    vk::DescriptorSetLayoutBinding::default()
        .binding(binding)
        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::COMPUTE)
}

pub(super) fn make_buffer(
    device: &ash::Device,
    properties: &vk::PhysicalDeviceMemoryProperties,
    size: u64,
) -> Option<(vk::Buffer, vk::DeviceMemory)> {
    let info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.create_buffer(&info, None) }.ok()?;
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
    let index = (0..properties.memory_type_count).find(|index| {
        let supported = requirements.memory_type_bits & (1 << index) != 0;
        let flags = properties.memory_types[*index as usize].property_flags;
        supported && flags.contains(wanted)
    })?;
    let memory = unsafe {
        device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(index),
            None,
        )
    }
    .ok()?;
    unsafe { device.bind_buffer_memory(buffer, memory, 0) }.ok()?;
    Some((buffer, memory))
}
