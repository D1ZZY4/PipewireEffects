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
            return;
        }
        let deadline_ns = block_budget_ns(buffer.frames(), self.format.rate());
        for slot in &mut self.slots {
            if !slot.enabled {
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
                continue;
            }
            slot.effect.process(buffer);
        }
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
mod tests {
    use super::*;

    struct Counting {
        id: &'static str,
        gain: f32,
    }

    impl Effect for Counting {
        fn id(&self) -> &'static str {
            self.id
        }
        fn prepare(&mut self, _format: AudioFormat) {}
        fn reset(&mut self) {}
        fn process(&mut self, buffer: &mut AudioBuffer) {
            for channel in buffer.channels_mut() {
                for sample in channel.iter_mut() {
                    *sample *= self.gain;
                }
            }
        }
    }

    fn effect(id: &'static str, gain: f32) -> Box<dyn Effect> {
        Box::new(Counting { id, gain })
    }

    fn buffer() -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, 4);
        buffer.configure(2, 2).unwrap();
        buffer.channel_mut(0).copy_from_slice(&[1.0, 1.0]);
        buffer.channel_mut(1).copy_from_slice(&[1.0, 1.0]);
        buffer
    }

    fn chain() -> EffectChain {
        EffectChain::new(4, AudioFormat::new(48_000, 2))
    }

    #[test]
    fn process_applies_effects_in_order() {
        let mut chain = chain();
        chain.push(effect("a", 2.0)).unwrap();
        chain.push(effect("b", 3.0)).unwrap();
        let mut buf = buffer();
        chain.process(&mut buf);
        assert!((buf.channel(0)[0] - 6.0).abs() < 1.0e-6);
    }

    #[test]
    fn bypass_skips_processing() {
        let mut chain = chain();
        chain.push(effect("a", 2.0)).unwrap();
        chain.set_bypassed(true);
        let mut buf = buffer();
        chain.process(&mut buf);
        assert!((buf.channel(0)[0] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn a_disabled_slot_is_skipped() {
        let mut chain = chain();
        chain.push(effect("a", 2.0)).unwrap();
        chain.push(effect("b", 3.0)).unwrap();
        let mut retired = Vec::new();
        chain.apply(
            ChainCommand::SetEnabled {
                index: 0,
                enabled: false,
            },
            |e| retired.push(e),
        );
        let mut buf = buffer();
        chain.process(&mut buf);
        assert!((buf.channel(0)[0] - 3.0).abs() < 1.0e-6);
        assert_eq!(chain.enabled(0), Some(false));
        assert_eq!(chain.enabled(9), None);
    }

    #[test]
    fn apply_matches_control_plane_ordering() {
        let mut chain = chain();
        let mut retired = Vec::new();
        chain.apply(ChainCommand::Add(effect("a", 1.0)), |e| retired.push(e));
        chain.apply(ChainCommand::Add(effect("b", 1.0)), |e| retired.push(e));
        chain.apply(ChainCommand::Add(effect("c", 1.0)), |e| retired.push(e));
        assert_eq!(chain.ids(), vec!["a", "b", "c"]);
        chain.apply(ChainCommand::Move { from: 0, to: 2 }, |e| retired.push(e));
        assert_eq!(chain.ids(), vec!["b", "c", "a"]);
        chain.apply(ChainCommand::Remove(1), |e| retired.push(e));
        assert_eq!(chain.ids(), vec!["b", "a"]);
        assert_eq!(retired.len(), 1);
    }

    #[test]
    fn an_accelerator_takes_over_the_elementwise_stage() {
        struct Doubler {
            calls: usize,
        }

        impl Accelerator for Doubler {
            fn scale(&mut self, buffer: &mut AudioBuffer, gain: f32, _deadline_ns: u64) -> bool {
                self.calls += 1;
                apply_gain(buffer, gain * 2.0);
                true
            }
        }

        struct Elementwise;

        impl Effect for Elementwise {
            fn id(&self) -> &'static str {
                "elementwise"
            }
            fn prepare(&mut self, _format: AudioFormat) {}
            fn reset(&mut self) {}
            fn process(&mut self, _buffer: &mut AudioBuffer) {
                panic!("the elementwise stage should have been off-loaded");
            }
            fn elementwise_gain(&mut self) -> Option<f32> {
                Some(0.5)
            }
        }

        let mut chain = chain();
        chain.push(Box::new(Elementwise)).unwrap();
        chain.set_accelerator(Some(Box::new(Doubler { calls: 0 })));
        let mut buf = buffer();
        chain.process(&mut buf);
        assert!((buf.channel(0)[0] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn a_declined_acceleration_falls_back_to_the_cpu() {
        struct Declining;

        impl Accelerator for Declining {
            fn scale(&mut self, _buffer: &mut AudioBuffer, _gain: f32, _deadline_ns: u64) -> bool {
                false
            }
        }

        struct Elementwise;

        impl Effect for Elementwise {
            fn id(&self) -> &'static str {
                "elementwise"
            }
            fn prepare(&mut self, _format: AudioFormat) {}
            fn reset(&mut self) {}
            fn process(&mut self, _buffer: &mut AudioBuffer) {}
            fn elementwise_gain(&mut self) -> Option<f32> {
                Some(0.5)
            }
        }

        let mut chain = chain();
        chain.push(Box::new(Elementwise)).unwrap();
        chain.set_accelerator(Some(Box::new(Declining)));
        let mut buf = buffer();
        chain.process(&mut buf);
        assert!((buf.channel(0)[0] - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn pushed_slots_start_enabled() {
        let mut chain = chain();
        chain.push(effect("a", 1.0)).unwrap();
        assert_eq!(chain.enabled(0), Some(true));
    }

    #[test]
    fn push_beyond_capacity_is_rejected() {
        let mut chain = EffectChain::new(1, AudioFormat::new(48_000, 2));
        chain.push(effect("a", 1.0)).unwrap();
        assert!(chain.push(effect("b", 1.0)).is_err());
        assert_eq!(chain.capacity(), 1);
    }
}
