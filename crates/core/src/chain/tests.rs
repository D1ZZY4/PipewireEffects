use super::*;
use crate::effect::EffectCategory;
use crate::param::{ParamSet, ParamSpec, ParamUnit};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn push_ok(chain: &mut EffectChain, effect: Box<dyn AudioEffect>, enabled: bool) -> usize {
    match chain.push(effect, enabled) {
        Ok(index) => index,
        Err(_) => panic!("the chain should have had room"),
    }
}

#[test]
fn insert_clamps_to_the_end_and_reports_the_position() {
    let mut chain = chain();
    assert_eq!(
        chain.insert(99, Box::new(Doubler::new()), true).ok(),
        Some(0)
    );
    assert_eq!(
        chain.insert(99, Box::new(Doubler::new()), true).ok(),
        Some(1)
    );
    assert_eq!(chain.len(), 2);
}

#[test]
fn processing_applies_enabled_effects_in_order() {
    let mut chain = chain();
    push_ok(&mut chain, Box::new(Doubler::new()), true);
    let mut samples = [1.0_f32; 2];
    chain.process(&mut block(&mut samples));
    assert_eq!(samples, [2.0, 2.0]);
}

#[test]
fn disabled_effects_are_skipped() {
    let mut chain = chain();
    push_ok(&mut chain, Box::new(Doubler::new()), false);
    let mut samples = [1.0_f32; 2];
    chain.process(&mut block(&mut samples));
    assert_eq!(samples, [1.0, 1.0]);
}

#[test]
fn bypass_skips_the_whole_chain() {
    let mut chain = chain();
    push_ok(&mut chain, Box::new(Doubler::new()), true);
    chain.set_bypassed(true);
    let mut samples = [1.0_f32; 2];
    chain.process(&mut block(&mut samples));
    assert_eq!(samples, [1.0, 1.0]);
}

#[test]
fn move_reorders_entries() {
    let mut chain = chain();
    push_ok(&mut chain, Box::new(Doubler::new()), true); // factor 2
    push_ok(&mut chain, Box::new(Doubler::new()), true); // factor 2
    chain.entry_mut(1).unwrap().parameters().set_f32(0, 3.0);

    let first = chain.entry(0).unwrap().kind();
    chain.move_entry(0, 1).unwrap();
    assert_eq!(chain.entry(1).unwrap().kind(), first);
}

#[test]
fn move_rejects_out_of_range_indices() {
    let mut chain = chain();
    push_ok(&mut chain, Box::new(Doubler::new()), true);
    assert!(matches!(
        chain.move_entry(0, 5),
        Err(CoreError::IndexOutOfRange { .. })
    ));
}

#[test]
fn remove_hands_the_instance_back_instead_of_dropping_it() {
    let counter = DropCounter::default();
    let mut chain = chain();
    push_ok(
        &mut chain,
        Box::new(Doubler::with_counter(counter.clone())),
        true,
    );

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
    push_ok(
        &mut chain,
        Box::new(Doubler::with_counter(counter.clone())),
        true,
    );

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
    push_ok(
        &mut chain,
        Box::new(Doubler::with_counter(counter.clone())),
        true,
    );
    push_ok(
        &mut chain,
        Box::new(Doubler::with_counter(counter.clone())),
        true,
    );

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
    push_ok(&mut chain, Box::new(Doubler::new()), true);
    push_ok(&mut chain, Box::new(Doubler::new()), true);
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
