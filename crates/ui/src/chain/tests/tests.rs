use super::*;

#[test]
fn a_snapshot_writes_only_the_parameters_that_differ_from_their_default() {
    let mut chain = Chain::new(None);
    chain
        .add("gain")
        .expect("the gain effect should be available");
    let mut section = Pipeline::default();
    chain.snapshot(&mut section);
    assert_eq!(section.effects.len(), 1);
    assert!(section.effects[0].params.is_empty());
}

#[test]
fn clearing_removes_every_slot_and_the_selection() {
    let mut chain = Chain::new(None);
    chain.add("gain").unwrap();
    chain.add("limiter").unwrap();
    assert_eq!(chain.slots.len(), 2);
    assert!(chain.selected.is_some());
    chain.clear();
    assert!(chain.slots.is_empty());
    assert!(chain.selected.is_none());
}
