#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use ash::vk;

use super::resources::{binding, make_buffer};

const SPIRV: &[u8] = include_bytes!("../scale.spv");

pub(super) const WORKGROUP: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Parameters {
    pub(super) factor: f32,
    pub(super) count: u32,
}

pub struct Scaler {
    pub(super) entry: ash::Entry,
    pub(super) instance: ash::Instance,
    pub(super) device: ash::Device,
    pub(super) queue: vk::Queue,
    pub(super) pipeline_layout: vk::PipelineLayout,
    pub(super) pipeline: vk::Pipeline,
    pub(super) set: vk::DescriptorSet,
    pub(super) command: vk::CommandBuffer,
    pub(super) fence: vk::Fence,
    pub(super) input: (vk::Buffer, vk::DeviceMemory),
    pub(super) output: (vk::Buffer, vk::DeviceMemory),
    pub(super) input_pointer: *mut f32,
    pub(super) output_pointer: *mut f32,
    pub(super) pending: bool,
    pub(super) failures: u32,
    pub(super) failed: bool,
    pub(super) capacity: usize,
}

pub(super) const MAX_CONSECUTIVE_FAILURES: u32 = 3;

unsafe impl Send for Scaler {}

impl Scaler {
    pub fn new(capacity: usize) -> Option<Self> {
        let capacity = capacity.max(1);
        let entry = unsafe { ash::Entry::load() }.ok()?;
        let app = vk::ApplicationInfo::default().api_version(vk::make_api_version(0, 1, 1, 0));
        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default().application_info(&app),
                None,
            )
        }
        .ok()?;
        let built = Self::build(entry, &instance, capacity);
        match built {
            Some(mut scaler) => {
                let mut warm = vec![0.0f32; capacity.min(256)];
                scaler.process(&mut warm, 1.0, 10_000_000);
                Some(scaler)
            }
            None => {
                unsafe { instance.destroy_instance(None) };
                None
            }
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    fn build(entry: ash::Entry, instance: &ash::Instance, capacity: usize) -> Option<Self> {
        let (physical, family) = super::resources::pick(instance)?;
        let priority = [1.0f32];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(family)
            .queue_priorities(&priority)];
        let device = unsafe {
            instance.create_device(
                physical,
                &vk::DeviceCreateInfo::default().queue_create_infos(&queues),
                None,
            )
        }
        .ok()?;
        let build = Self::build_on(entry, instance, &device, physical, family, capacity);
        match build {
            Some(scaler) => Some(scaler),
            None => {
                unsafe { device.destroy_device(None) };
                None
            }
        }
    }

    fn build_on(
        entry: ash::Entry,
        instance: &ash::Instance,
        device: &ash::Device,
        physical: vk::PhysicalDevice,
        family: u32,
        capacity: usize,
    ) -> Option<Self> {
        let properties = unsafe { instance.get_physical_device_memory_properties(physical) };
        let size = (capacity * 4) as u64;
        let input = make_buffer(device, &properties, size)?;
        let output = make_buffer(device, &properties, size)?;

        let bindings = [binding(0), binding(1)];
        let set_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }
        .ok()?;
        let range = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .offset(0)
            .size(size_of::<Parameters>() as u32)];
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(std::slice::from_ref(&set_layout))
                    .push_constant_ranges(&range),
                None,
            )
        }
        .ok()?;
        let code = ash::util::read_spv(&mut std::io::Cursor::new(SPIRV)).ok()?;
        let shader = unsafe {
            device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)
        }
        .ok()?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader)
            .name(c"main");
        let pipeline_info = vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(pipeline_layout);
        let pipelines = unsafe {
            device.create_compute_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
        }
        .ok()?;
        let pipeline = *pipelines.first()?;

        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(2)];
        let pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .pool_sizes(&sizes)
                    .max_sets(1),
                None,
            )
        }
        .ok()?;
        let set = *unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(std::slice::from_ref(&set_layout)),
            )
        }
        .ok()?
        .first()?;
        let input_info = [vk::DescriptorBufferInfo::default()
            .buffer(input.0)
            .offset(0)
            .range(vk::WHOLE_SIZE)];
        let output_info = [vk::DescriptorBufferInfo::default()
            .buffer(output.0)
            .offset(0)
            .range(vk::WHOLE_SIZE)];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&input_info),
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&output_info),
        ];
        unsafe { device.update_descriptor_sets(&writes, &[]) };

        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(family),
                None,
            )
        }
        .ok()?;
        let command = *unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .ok()?
        .first()?;
        let fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }.ok()?;

        let input_pointer = unsafe {
            device
                .map_memory(input.1, 0, size, vk::MemoryMapFlags::empty())
                .ok()?
                .cast::<f32>()
        };
        let output_pointer =
            match unsafe { device.map_memory(output.1, 0, size, vk::MemoryMapFlags::empty()) } {
                Ok(pointer) => pointer.cast::<f32>(),
                Err(_) => {
                    unsafe { device.unmap_memory(input.1) };
                    return None;
                }
            };

        Some(Self {
            entry,
            instance: ash::Instance::clone(instance),
            device: ash::Device::clone(device),
            queue: unsafe { device.get_device_queue(family, 0) },
            pipeline_layout,
            pipeline,
            set,
            command,
            fence,
            input,
            output,
            input_pointer,
            output_pointer,
            pending: false,
            failures: 0,
            failed: false,
            capacity,
        })
    }
}
