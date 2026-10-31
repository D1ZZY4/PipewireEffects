use crate::audio::{AudioBuffer, AudioFormat};
use crate::command::ChainCommand;
use crate::effect::{Accelerator, Effect};
use crate::error::{CoreError, CoreResult};

struct Slot {
    effect: Box<dyn Effect>,
    enabled: bool,
}

pub struct EffectChain {
    slots: Vec<Slot>,
    capacity: usize,
    bypassed: bool,
    format: AudioFormat,
    accelerator: Option<Box<dyn Accelerator>>,
    peaks: Vec<f32>,
}

impl EffectChain {
    pub fn new(capacity: usize, format: AudioFormat) -> Self {
        let capacity = capacity.max(1);
        Self {
            slots: Vec::with_capacity(capacity),
            capacity,
            bypassed: false,
            format,
            accelerator: None,
            peaks: vec![0.0; capacity],
        }
    }

    pub fn set_accelerator(&mut self, accelerator: Option<Box<dyn Accelerator>>) {
        self.accelerator = accelerator;
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.slots.iter().map(|slot| slot.effect.id()).collect()
    }

    pub fn enabled(&self, index: usize) -> Option<bool> {
        self.slots.get(index).map(|slot| slot.enabled)
    }

    pub fn latency_frames(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.enabled)
            .map(|slot| slot.effect.latency_frames())
            .sum()
    }

    pub fn push(&mut self, mut effect: Box<dyn Effect>) -> CoreResult<()> {
        if self.slots.len() >= self.capacity {
            return Err(CoreError::ChainFull {
                capacity: self.capacity,
            });
        }
        effect.prepare(self.format);
        self.slots.push(Slot {
            effect,
            enabled: true,
        });
        Ok(())
    }

    pub fn prepare(&mut self, format: AudioFormat) {
        self.format = format;
        for slot in &mut self.slots {
            slot.effect.prepare(format);
        }
    }

    pub fn process(&mut self, buffer: &mut AudioBuffer) {
        if self.bypassed {
            self.peaks.fill(0.0);
            return;
        }
        let deadline_ns = block_budget_ns(buffer.frames(), self.format.rate());
        let peaks = &mut self.peaks;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !slot.enabled {
                if let Some(peak) = peaks.get_mut(index) {
                    *peak = 0.0;
                }
                continue;
            }
            if let Some(gain) = slot.effect.elementwise_gain() {
                let offloaded = self
                    .accelerator
                    .as_mut()
                    .is_some_and(|accelerator| accelerator.scale(buffer, gain, deadline_ns));
                if !offloaded {
                    apply_gain(buffer, gain);
                }
            } else {
                slot.effect.process(buffer);
            }
            if let Some(peak) = peaks.get_mut(index) {
                *peak = buffer.peak();
            }
        }
        for peak in peaks.iter_mut().skip(self.slots.len()) {
            *peak = 0.0;
        }
    }

    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }

    pub fn apply<F>(&mut self, command: ChainCommand, mut retire: F)
    where
        F: FnMut(Box<dyn Effect>),
    {
        match command {
            ChainCommand::Add(mut effect) => {
                if self.slots.len() < self.capacity {
                    effect.prepare(self.format);
                    self.slots.push(Slot {
                        effect,
                        enabled: true,
                    });
                } else {
                    retire(effect);
                }
            }
            ChainCommand::Remove(index) => {
                if index < self.slots.len() {
                    let removed = self.slots.remove(index);
                    retire(removed.effect);
                }
            }
            ChainCommand::Move { from, to } => {
                let len = self.slots.len();
                if from < len && to < len && from != to {
                    let slot = self.slots.remove(from);
                    self.slots.insert(to, slot);
                }
            }
            ChainCommand::SetEnabled { index, enabled } => {
                if let Some(slot) = self.slots.get_mut(index) {
                    slot.enabled = enabled;
                }
            }
            ChainCommand::SetBypass(state) => {
                self.bypassed = state;
            }
        }
    }
}

fn apply_gain(buffer: &mut AudioBuffer, gain: f32) {
    for channel in buffer.channels_mut() {
        for sample in channel {
            *sample *= gain;
        }
    }
}

fn block_budget_ns(frames: usize, rate: u32) -> u64 {
    let frames = frames as u64;
    let rate = u64::from(rate.max(8_000));
    frames.saturating_mul(1_000_000_000) / rate / 2
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
