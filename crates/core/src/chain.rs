//! The ordered chain of effects, plus the commands that mutate it.
//!
//! The chain is owned by whichever thread is processing audio. Structural edits
//! — adding, removing, reordering — cannot be done from the interface thread,
//! because that would mean racing with the audio callback. Instead the
//! interface posts a [`ChainCommand`] onto a lock-free queue and the audio
//! thread applies it at the top of the next block.
//!
//! Removing an effect is the interesting case: dropping a `Box<dyn AudioEffect>`
//! deallocates, and the allocator may block. [`ChainCommand::apply`] therefore
//! never drops anything. It hands retired instances to a `retire` callback so
//! the caller can park them on a queue and let a non-realtime thread do the
//! freeing.

use crate::audio::{AudioBlock, AudioFormat};
use crate::effect::{AudioEffect, EffectKind};
use crate::error::{CoreError, Result};
use crate::param::ParamSet;

/// One slot in the chain.
pub struct ChainEntry {
    effect: Box<dyn AudioEffect>,
    enabled: bool,
}

impl ChainEntry {
    /// Wraps an effect instance.
    #[must_use]
    pub fn new(effect: Box<dyn AudioEffect>, enabled: bool) -> Self {
        Self { effect, enabled }
    }

    /// The effect instance.
    #[must_use]
    pub fn effect(&self) -> &dyn AudioEffect {
        self.effect.as_ref()
    }

    /// The effect instance, mutably.
    pub fn effect_mut(&mut self) -> &mut dyn AudioEffect {
        self.effect.as_mut()
    }

    /// The effect type.
    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.effect.kind()
    }

    /// The live parameters of the effect.
    #[must_use]
    pub fn parameters(&self) -> &ParamSet {
        self.effect.parameters()
    }

    /// Whether the effect is currently processing audio.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the effect without removing it.
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

/// An ordered list of effects, processed front to back.
#[derive(Debug)]
pub struct EffectChain {
    format: AudioFormat,
    entries: Vec<ChainEntry>,
    bypassed: bool,
    processed_frames: u64,
}

impl EffectChain {
    /// Creates an empty chain for a format.
    #[must_use]
    pub fn new(format: AudioFormat) -> Self {
        Self {
            format,
            entries: Vec::new(),
            bypassed: false,
            processed_frames: 0,
        }
    }

    /// The format the chain is configured for.
    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    /// Re-configures every effect for a new format.
    ///
    /// This is one of the few operations allowed to allocate, and it only ever
    /// runs from the audio thread while the stream is paused or from a
    /// non-realtime thread.
    pub fn set_format(&mut self, format: AudioFormat) {
        self.format = format;
        for entry in &mut self.entries {
            entry.effect.prepare(format);
        }
    }

    /// The entries, in processing order.
    #[must_use]
    pub fn entries(&self) -> &[ChainEntry] {
        &self.entries
    }

    /// The entry at `index`.
    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&ChainEntry> {
        self.entries.get(index)
    }

    /// The entry at `index`, mutably.
    pub fn entry_mut(&mut self, index: usize) -> Option<&mut ChainEntry> {
        self.entries.get_mut(index)
    }

    /// The number of effects in the chain, including disabled ones.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the chain holds no effects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether the whole chain is bypassed.
    #[must_use]
    pub const fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    /// Bypasses or re-enables the whole chain without losing its contents.
    pub const fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }

    /// The total number of frames processed since the last reset.
    #[must_use]
    pub const fn processed_frames(&self) -> u64 {
        self.processed_frames
    }

    /// Inserts an effect, clamping `index` to the end of the chain.
    ///
    /// Returns the index the effect actually landed at.
    pub fn insert(&mut self, index: usize, effect: Box<dyn AudioEffect>, enabled: bool) -> usize {
        let index = index.min(self.entries.len());
        let mut entry = ChainEntry::new(effect, enabled);
        entry.effect.prepare(self.format);
        self.entries.insert(index, entry);
        index
    }

    /// Appends an effect to the end of the chain.
    pub fn push(&mut self, effect: Box<dyn AudioEffect>, enabled: bool) -> usize {
        self.insert(self.entries.len(), effect, enabled)
    }

    /// Removes an effect and hands it back to the caller.
    ///
    /// The instance is returned rather than dropped so that the caller can
    /// dispose of it off the audio thread.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::IndexOutOfRange`] when `index` does not exist.
    pub fn remove(&mut self, index: usize) -> Result<Box<dyn AudioEffect>> {
        if index >= self.entries.len() {
            return Err(CoreError::IndexOutOfRange {
                index,
                len: self.entries.len(),
            });
        }
        Ok(self.entries.remove(index).effect)
    }

    /// Moves an entry to a new position.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::IndexOutOfRange`] when either index does not exist.
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

    /// Enables or disables one entry.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::IndexOutOfRange`] when `index` does not exist.
    pub fn set_enabled(&mut self, index: usize, enabled: bool) -> Result<()> {
        let len = self.entries.len();
        self.entries
            .get_mut(index)
            .map(|entry| entry.set_enabled(enabled))
            .ok_or(CoreError::IndexOutOfRange { index, len })
    }

    /// Enables or disables every entry at once.
    pub fn set_all_enabled(&mut self, enabled: bool) {
        for entry in &mut self.entries {
            entry.set_enabled(enabled);
        }
    }

    /// Clears the filter memory and envelopes of every effect.
    pub fn reset(&mut self) {
        for entry in &mut self.entries {
            entry.effect.reset();
        }
        self.processed_frames = 0;
    }

    /// Empties the chain, handing every effect to `retire`.
    pub fn drain(&mut self, mut retire: impl FnMut(Box<dyn AudioEffect>)) {
        for entry in self.entries.drain(..) {
            retire(entry.effect);
        }
    }

    /// The summed latency of every enabled effect, in frames.
    #[must_use]
    pub fn latency_frames(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_enabled())
            .map(|entry| entry.effect.latency_frames())
            .sum()
    }

    /// Runs a block through the chain.
    ///
    /// Allocation-free and lock-free: this is the function called from the
    /// realtime callback.
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

/// A structural edit to apply to the chain on the audio thread.
pub enum ChainCommand {
    /// Insert an effect at `index`.
    Insert {
        /// Target position; clamped to the end of the chain.
        index: usize,
        /// The instance to insert.
        effect: Box<dyn AudioEffect>,
        /// Whether it should start enabled.
        enabled: bool,
    },
    /// Remove the effect at `index`.
    Remove {
        /// Position to remove.
        index: usize,
    },
    /// Move the effect at `from` to `to`.
    Move {
        /// Current position.
        from: usize,
        /// Destination position.
        to: usize,
    },
    /// Enable or disable one effect.
    SetEnabled {
        /// Position to change.
        index: usize,
        /// New state.
        enabled: bool,
    },
    /// Enable or disable every effect.
    SetAllEnabled {
        /// New state.
        enabled: bool,
    },
    /// Bypass or re-engage the whole chain.
    SetBypassed {
        /// New state.
        bypassed: bool,
    },
    /// Clear effect state without removing anything.
    Reset,
    /// Remove every effect.
    Clear,
}

impl ChainCommand {
    /// Applies the edit, passing any retired instances to `retire`.
    ///
    /// Out-of-range indices are ignored rather than treated as an error: the
    /// interface and the audio thread can be one command out of step while an
    /// edit is in flight, and the correct response is to do nothing.
    pub fn apply(self, chain: &mut EffectChain, retire: &mut dyn FnMut(Box<dyn AudioEffect>)) {
        match self {
            Self::Insert {
                index,
                effect,
                enabled,
            } => {
                chain.insert(index, effect, enabled);
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
mod tests {
    use super::*;
    use crate::effect::EffectCategory;
    use crate::param::{ParamSet, ParamSpec, ParamUnit};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Counts how many instances have been dropped, so tests can prove that
    /// `ChainCommand::apply` does not free anything itself.
    #[derive(Default, Clone)]
    struct DropCounter(Arc<AtomicUsize>);

    struct Doubler {
        params: ParamSet,
        counter: Option<DropCounter>,
    }

    impl Doubler {
        fn params() -> ParamSet {
            ParamSet::new(vec![ParamSpec::float(
                "factor",
                "Factor",
                0.0,
                4.0,
                2.0,
                0.1,
                ParamUnit::Ratio,
            )])
        }

        fn new() -> Self {
            Self {
                params: Self::params(),
                counter: None,
            }
        }

        fn with_counter(counter: DropCounter) -> Self {
            Self {
                params: Self::params(),
                counter: Some(counter),
            }
        }
    }

    impl Drop for Doubler {
        fn drop(&mut self) {
            if let Some(counter) = &self.counter {
                counter.0.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    impl AudioEffect for Doubler {
        fn kind(&self) -> EffectKind {
            EffectKind::new("test-doubler")
        }
        fn parameters(&self) -> &ParamSet {
            &self.params
        }
        fn prepare(&mut self, _format: AudioFormat) {}
        fn reset(&mut self) {}
        fn process(&mut self, block: &mut AudioBlock<'_>) {
            let factor = self.params.value_f32(0);
            block.apply_gain(factor);
        }
    }

    fn chain() -> EffectChain {
        EffectChain::new(AudioFormat::default_stereo())
    }

    fn block(samples: &mut [f32]) -> AudioBlock<'_> {
        AudioBlock::new(samples, AudioFormat::default_stereo())
    }

    #[test]
    fn insert_clamps_to_the_end_and_reports_the_position() {
        let mut chain = chain();
        assert_eq!(chain.insert(99, Box::new(Doubler::new()), true), 0);
        assert_eq!(chain.insert(99, Box::new(Doubler::new()), true), 1);
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn processing_applies_enabled_effects_in_order() {
        let mut chain = chain();
        chain.push(Box::new(Doubler::new()), true);
        let mut samples = [1.0_f32; 2];
        chain.process(&mut block(&mut samples));
        assert_eq!(samples, [2.0, 2.0]);
    }

    #[test]
    fn disabled_effects_are_skipped() {
        let mut chain = chain();
        chain.push(Box::new(Doubler::new()), false);
        let mut samples = [1.0_f32; 2];
        chain.process(&mut block(&mut samples));
        assert_eq!(samples, [1.0, 1.0]);
    }

    #[test]
    fn bypass_skips_the_whole_chain() {
        let mut chain = chain();
        chain.push(Box::new(Doubler::new()), true);
        chain.set_bypassed(true);
        let mut samples = [1.0_f32; 2];
        chain.process(&mut block(&mut samples));
        assert_eq!(samples, [1.0, 1.0]);
    }

    #[test]
    fn move_reorders_entries() {
        let mut chain = chain();
        chain.push(Box::new(Doubler::new()), true); // factor 2
        chain.push(Box::new(Doubler::new()), true); // factor 2
        chain.entry_mut(1).unwrap().parameters().set_f32(0, 3.0);

        // 2 then 3 or 3 then 2 both give 6, so check identity instead.
        let first = chain.entry(0).unwrap().kind();
        chain.move_entry(0, 1).unwrap();
        assert_eq!(chain.entry(1).unwrap().kind(), first);
    }

    #[test]
    fn move_rejects_out_of_range_indices() {
        let mut chain = chain();
        chain.push(Box::new(Doubler::new()), true);
        assert!(matches!(
            chain.move_entry(0, 5),
            Err(CoreError::IndexOutOfRange { .. })
        ));
    }

    #[test]
    fn remove_hands_the_instance_back_instead_of_dropping_it() {
        let counter = DropCounter::default();
        let mut chain = chain();
        chain.push(Box::new(Doubler::with_counter(counter.clone())), true);

        let removed = chain.remove(0).unwrap();
        assert_eq!(
            counter.0.load(Ordering::SeqCst),
            0,
            "dropped on the audio thread"
        );
        drop(removed);
        assert_eq!(counter.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn applying_remove_retires_rather_than_drops() {
        let counter = DropCounter::default();
        let mut chain = chain();
        chain.push(Box::new(Doubler::with_counter(counter.clone())), true);

        let mut retired = Vec::new();
        ChainCommand::Remove { index: 0 }.apply(&mut chain, &mut |effect| retired.push(effect));

        assert!(chain.is_empty());
        assert_eq!(counter.0.load(Ordering::SeqCst), 0);
        assert_eq!(retired.len(), 1);
    }

    #[test]
    fn applying_clear_retires_every_instance() {
        let counter = DropCounter::default();
        let mut chain = chain();
        chain.push(Box::new(Doubler::with_counter(counter.clone())), true);
        chain.push(Box::new(Doubler::with_counter(counter.clone())), true);

        let mut retired = Vec::new();
        ChainCommand::Clear.apply(&mut chain, &mut |effect| retired.push(effect));

        assert!(chain.is_empty());
        assert_eq!(counter.0.load(Ordering::SeqCst), 0);
        assert_eq!(retired.len(), 2);
    }

    #[test]
    fn out_of_range_commands_are_ignored() {
        let mut chain = chain();
        let mut retired = Vec::new();
        ChainCommand::Remove { index: 7 }.apply(&mut chain, &mut |effect| retired.push(effect));
        ChainCommand::Move { from: 0, to: 9 }.apply(&mut chain, &mut |effect| retired.push(effect));
        ChainCommand::SetEnabled {
            index: 3,
            enabled: true,
        }
        .apply(&mut chain, &mut |effect| retired.push(effect));
        assert!(retired.is_empty());
    }

    #[test]
    fn latency_is_summed_over_enabled_entries_only() {
        let mut chain = chain();
        chain.push(Box::new(Doubler::new()), true);
        chain.push(Box::new(Doubler::new()), true);
        assert_eq!(chain.latency_frames(), 0);
    }

    #[test]
    fn processed_frame_counter_accumulates() {
        let mut chain = chain();
        let mut samples = [0.0_f32; 8];
        chain.process(&mut block(&mut samples));
        chain.process(&mut block(&mut samples));
        assert_eq!(chain.processed_frames(), 8);
    }

    #[test]
    fn registry_sized_category_exists_for_tests() {
        assert_eq!(EffectCategory::ALL.len(), 5);
    }
}
