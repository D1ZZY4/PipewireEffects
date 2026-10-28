use crate::audio::{AudioBlock, AudioFormat};
use crate::effect::{AudioEffect, EffectKind};
use crate::error::{CoreError, Result};
use crate::param::ParamSet;

pub struct ChainEntry {
    effect: Box<dyn AudioEffect>,
    enabled: bool,
}

impl ChainEntry {
    #[must_use]
    pub fn new(effect: Box<dyn AudioEffect>, enabled: bool) -> Self {
        Self { effect, enabled }
    }

    #[must_use]
    pub fn effect(&self) -> &dyn AudioEffect {
        self.effect.as_ref()
    }

    pub fn effect_mut(&mut self) -> &mut dyn AudioEffect {
        self.effect.as_mut()
    }

    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.effect.kind()
    }

    #[must_use]
    pub fn parameters(&self) -> &ParamSet {
        self.effect.parameters()
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl std::fmt::Debug for ChainEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainEntry")
            .field("kind", &self.kind())
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

pub const CHAIN_CAPACITY: usize = 32;

#[derive(Debug)]
pub struct EffectChain {
    format: AudioFormat,
    entries: Vec<ChainEntry>,
    bypassed: bool,
    processed_frames: u64,
}

impl EffectChain {
    #[must_use]
    pub fn new(format: AudioFormat) -> Self {
        Self {
            format,
            entries: Vec::with_capacity(CHAIN_CAPACITY),
            bypassed: false,
            processed_frames: 0,
        }
    }

    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn set_format(&mut self, format: AudioFormat) {
        self.format = format;
        for entry in &mut self.entries {
            entry.effect.prepare(format);
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[ChainEntry] {
        &self.entries
    }

    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&ChainEntry> {
        self.entries.get(index)
    }

    pub fn entry_mut(&mut self, index: usize) -> Option<&mut ChainEntry> {
        self.entries.get_mut(index)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub const fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    pub const fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }

    #[must_use]
    pub const fn processed_frames(&self) -> u64 {
        self.processed_frames
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        CHAIN_CAPACITY
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.entries.len() >= CHAIN_CAPACITY
    }

    pub fn insert(
        &mut self,
        index: usize,
        effect: Box<dyn AudioEffect>,
        enabled: bool,
    ) -> Result<usize, Box<dyn AudioEffect>> {
        if self.is_full() {
            return Err(effect);
        }
        let index = index.min(self.entries.len());
        self.entries.insert(index, ChainEntry::new(effect, enabled));
        Ok(index)
    }

    pub fn push(
        &mut self,
        effect: Box<dyn AudioEffect>,
        enabled: bool,
    ) -> Result<usize, Box<dyn AudioEffect>> {
        self.insert(self.entries.len(), effect, enabled)
    }

    pub fn remove(&mut self, index: usize) -> Result<Box<dyn AudioEffect>> {
        if index >= self.entries.len() {
            return Err(CoreError::IndexOutOfRange {
                index,
                len: self.entries.len(),
            });
        }
        Ok(self.entries.remove(index).effect)
    }

    pub fn move_entry(&mut self, from: usize, to: usize) -> Result<()> {
        let len = self.entries.len();
        if from >= len || to >= len {
            return Err(CoreError::IndexOutOfRange {
                index: from.max(to),
                len,
            });
        }
        if from != to {
            let entry = self.entries.remove(from);
            self.entries.insert(to, entry);
        }
        Ok(())
    }

    pub fn set_enabled(&mut self, index: usize, enabled: bool) -> Result<()> {
        let len = self.entries.len();
        self.entries
            .get_mut(index)
            .map(|entry| entry.set_enabled(enabled))
            .ok_or(CoreError::IndexOutOfRange { index, len })
    }

    pub fn set_all_enabled(&mut self, enabled: bool) {
        for entry in &mut self.entries {
            entry.set_enabled(enabled);
        }
    }

    pub fn reset(&mut self) {
        for entry in &mut self.entries {
            entry.effect.reset();
        }
        self.processed_frames = 0;
    }

    pub fn drain(&mut self, mut retire: impl FnMut(Box<dyn AudioEffect>)) {
        for entry in self.entries.drain(..) {
            retire(entry.effect);
        }
    }

    #[must_use]
    pub fn latency_frames(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_enabled())
            .map(|entry| entry.effect.latency_frames())
            .sum()
    }

    pub fn process(&mut self, block: &mut AudioBlock<'_>) {
        if self.bypassed {
            return;
        }
        for entry in &mut self.entries {
            if entry.enabled {
                entry.effect.process(block);
            }
        }
        self.processed_frames = self
            .processed_frames
            .saturating_add(u64::try_from(block.frames()).unwrap_or(u64::MAX));
    }
}

pub enum ChainCommand {
    Insert {
        index: usize,
        effect: Box<dyn AudioEffect>,
        enabled: bool,
    },
    Remove {
        index: usize,
    },
    Move {
        from: usize,
        to: usize,
    },
    SetEnabled {
        index: usize,
        enabled: bool,
    },
    SetAllEnabled {
        enabled: bool,
    },
    SetBypassed {
        bypassed: bool,
    },
    Reset,
    Clear,
}

impl ChainCommand {
    pub fn apply(self, chain: &mut EffectChain, retire: &mut dyn FnMut(Box<dyn AudioEffect>)) {
        match self {
            Self::Insert {
                index,
                effect,
                enabled,
            } => {
                if let Err(overflow) = chain.insert(index, effect, enabled) {
                    retire(overflow);
                }
            }
            Self::Remove { index } => {
                if let Ok(effect) = chain.remove(index) {
                    retire(effect);
                }
            }
            Self::Move { from, to } => {
                let _ = chain.move_entry(from, to);
            }
            Self::SetEnabled { index, enabled } => {
                let _ = chain.set_enabled(index, enabled);
            }
            Self::SetAllEnabled { enabled } => chain.set_all_enabled(enabled),
            Self::SetBypassed { bypassed } => chain.set_bypassed(bypassed),
            Self::Reset => chain.reset(),
            Self::Clear => chain.drain(&mut *retire),
        }
    }
}

impl std::fmt::Debug for ChainCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Insert { index, enabled, .. } => f
                .debug_struct("Insert")
                .field("index", index)
                .field("enabled", enabled)
                .finish_non_exhaustive(),
            Self::Remove { index } => f.debug_struct("Remove").field("index", index).finish(),
            Self::Move { from, to } => f
                .debug_struct("Move")
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::SetEnabled { index, enabled } => f
                .debug_struct("SetEnabled")
                .field("index", index)
                .field("enabled", enabled)
                .finish(),
            Self::SetAllEnabled { enabled } => f
                .debug_struct("SetAllEnabled")
                .field("enabled", enabled)
                .finish(),
            Self::SetBypassed { bypassed } => f
                .debug_struct("SetBypassed")
                .field("bypassed", bypassed)
                .finish(),
            Self::Reset => f.write_str("Reset"),
            Self::Clear => f.write_str("Clear"),
        }
    }
}

#[cfg(test)]
mod tests;
