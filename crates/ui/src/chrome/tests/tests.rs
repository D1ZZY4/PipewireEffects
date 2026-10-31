use super::is_narrow;

#[test]
fn the_narrow_decision_covers_the_breakpoint() {
    assert!(is_narrow(360));
    assert!(is_narrow(720));
    assert!(is_narrow(860));
    assert!(!is_narrow(861));
    assert!(!is_narrow(1040));
}

#[test]
fn an_unallocated_width_is_not_narrow() {
    assert!(!is_narrow(0));
    assert!(!is_narrow(-1));
}
