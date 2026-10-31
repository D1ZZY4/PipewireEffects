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

#[test]
fn the_chain_reports_the_peak_of_each_slot() {
    let mut chain = chain();
    chain.push(effect("a", 2.0)).unwrap();
    chain.push(effect("b", 0.5)).unwrap();
    let mut buf = buffer();
    chain.process(&mut buf);
    assert!((chain.peaks()[0] - 2.0).abs() < 1.0e-6);
    assert!((chain.peaks()[1] - 1.0).abs() < 1.0e-6);
}

#[test]
fn a_bypassed_chain_reports_no_slot_levels() {
    let mut chain = chain();
    chain.push(effect("a", 2.0)).unwrap();
    chain.set_bypassed(true);
    let mut buf = buffer();
    chain.process(&mut buf);
    assert_eq!(chain.peaks()[0], 0.0);
}

#[test]
fn a_disabled_slot_reports_no_level() {
    let mut chain = chain();
    chain.push(effect("a", 2.0)).unwrap();
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
    assert_eq!(chain.peaks()[0], 0.0);
}
