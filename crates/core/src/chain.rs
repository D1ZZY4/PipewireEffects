use crate::audio::{AudioBuffer, AudioFormat};
use crate::command::ChainCommand;
use crate::effect::Effect;
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
}

impl EffectChain {
    pub fn new(capacity: usize, format: AudioFormat) -> Self {
        let capacity = capacity.max(1);
        Self {
            slots: Vec::with_capacity(capacity),
            capacity,
            bypassed: false,
            format,
        }
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
        for slot in &mut self.slots {
            if slot.enabled {
                slot.effect.process(buffer);
            }
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
