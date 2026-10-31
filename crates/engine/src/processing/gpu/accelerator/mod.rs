#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::sync::Arc;

use pwe_core::AudioBuffer;

use super::Scaler;
use crate::shared::Shared;

pub struct Accelerator {
    scaler: Scaler,
    shared: Arc<Shared>,
    verify: bool,
    scratch: Vec<f32>,
    reference: Vec<f32>,
}

impl Accelerator {
    pub fn new(capacity: usize, verify: bool, shared: Arc<Shared>) -> Option<Self> {
        let scaler = Scaler::new(capacity)?;
        Some(Self {
            scaler,
            shared,
            verify,
            scratch: vec![0.0; capacity],
            reference: vec![0.0; capacity],
        })
    }
}

impl pwe_core::Accelerator for Accelerator {
    fn scale(&mut self, buffer: &mut AudioBuffer, gain: f32, deadline_ns: u64) -> bool {
        let frames = buffer.frames();
        let count = frames * buffer.channels();
        if count == 0 {
            return true;
        }
        if count > self.scratch.len() {
            return false;
        }
        let mut offset = 0;
        for channel in buffer.channels_mut() {
            self.scratch[offset..offset + frames].copy_from_slice(channel);
            offset += frames;
        }
        if self.verify {
            self.reference[..count].copy_from_slice(&self.scratch[..count]);
            for sample in &mut self.reference[..count] {
                *sample *= gain;
            }
        }
        if !self
            .scaler
            .process(&mut self.scratch[..count], gain, deadline_ns)
        {
            self.shared.stats().record_gpu_fallback();
            return false;
        }
        if self.verify {
            let agreed = self.scratch[..count]
                .iter()
                .zip(&self.reference[..count])
                .all(|(gpu, cpu)| (gpu - cpu).abs() <= 1.0e-5_f32.max(cpu.abs() * 1.0e-5));
            if !agreed {
                self.scratch[..count].copy_from_slice(&self.reference[..count]);
                self.shared.stats().record_gpu_disagreement();
            }
        }
        let mut offset = 0;
        for channel in buffer.channels_mut() {
            channel.copy_from_slice(&self.scratch[offset..offset + frames]);
            offset += frames;
        }
        self.shared.stats().record_gpu_block();
        true
    }
}
