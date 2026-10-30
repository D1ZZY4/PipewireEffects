use ash::vk;

use super::scaler::{MAX_CONSECUTIVE_FAILURES, Parameters, Scaler, WORKGROUP};

const WAIT_FLOOR_NS: u64 = 100_000;
const WAIT_CEILING_NS: u64 = 25_000_000;

impl Scaler {
    pub fn process(&mut self, samples: &mut [f32], factor: f32, deadline_ns: u64) -> bool {
        if self.failed || samples.is_empty() || samples.len() > self.capacity {
            return false;
        }
        let wait_ns = deadline_ns.clamp(WAIT_FLOOR_NS, WAIT_CEILING_NS);
        if !self.finish(wait_ns) {
            return false;
        }
        let count = samples.len();
        let parameters = Parameters {
            factor,
            count: count as u32,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                std::ptr::from_ref(&parameters).cast::<u8>(),
                size_of::<Parameters>(),
            )
        };
        unsafe {
            std::slice::from_raw_parts_mut(self.input_pointer, count).copy_from_slice(samples);
        }
        let recorded = unsafe {
            let device = &self.device;
            (|| {
                device.reset_command_buffer(self.command, vk::CommandBufferResetFlags::empty())?;
                device
                    .begin_command_buffer(self.command, &vk::CommandBufferBeginInfo::default())?;
                device.cmd_bind_pipeline(
                    self.command,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipeline,
                );
                device.cmd_bind_descriptor_sets(
                    self.command,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipeline_layout,
                    0,
                    std::slice::from_ref(&self.set),
                    &[],
                );
                device.cmd_push_constants(
                    self.command,
                    self.pipeline_layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    bytes,
                );
                device.cmd_dispatch(self.command, (count as u32).div_ceil(WORKGROUP), 1, 1);
                device.end_command_buffer(self.command)
            })()
        };
        if recorded.is_err() {
            self.failed = true;
            return false;
        }
        let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&self.command));
        if unsafe {
            self.device
                .queue_submit(self.queue, &[submit], self.fence)
                .is_err()
        } {
            self.failed = true;
            return false;
        }
        self.pending = true;
        if !self.finish(wait_ns) {
            return false;
        }
        unsafe {
            samples.copy_from_slice(std::slice::from_raw_parts(self.output_pointer, count));
        }
        true
    }

    fn finish(&mut self, wait_ns: u64) -> bool {
        if !self.pending {
            return true;
        }
        match unsafe { self.device.wait_for_fences(&[self.fence], true, wait_ns) } {
            Ok(()) => {
                let _ = unsafe { self.device.reset_fences(&[self.fence]) };
                self.failures = 0;
                self.pending = false;
                true
            }
            Err(vk::Result::TIMEOUT) => {
                self.failures += 1;
                if self.failures >= MAX_CONSECUTIVE_FAILURES {
                    self.failed = true;
                }
                false
            }
            Err(_) => {
                self.failed = true;
                false
            }
        }
    }
}

impl Drop for Scaler {
    fn drop(&mut self) {
        let _ = &self.entry;
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.unmap_memory(self.input.1);
            self.device.unmap_memory(self.output.1);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

pub fn scale(values: &[f32], factor: f32) -> Option<Vec<f32>> {
    let mut scaler = Scaler::new(values.len())?;
    let mut out = values.to_vec();
    if scaler.process(&mut out, factor, 1_000_000_000) {
        Some(out)
    } else {
        None
    }
}
