use super::*;

#[test]
fn a_comb_echoes_after_its_length() {
    let mut comb = Comb::new(16);
    comb.set_length(4);
    comb.feedback = 0.5;
    comb.damping = 0.0;
    assert_eq!(comb.process(1.0), 0.0);
    for _ in 0..3 {
        assert_eq!(comb.process(0.0), 0.0);
    }
    let echo = comb.process(0.0);
    assert!((echo - 1.0).abs() < 1.0e-6, "{echo}");
}

#[test]
fn an_allpass_returns_the_buffered_sample_after_its_length() {
    let mut allpass = AllPass::new(16);
    allpass.set_length(4);
    allpass.feedback = 0.5;
    assert!((allpass.process(1.0) + 1.0).abs() < 1.0e-6);
    for _ in 0..3 {
        assert_eq!(allpass.process(0.0), 0.0);
    }
    let reflection = allpass.process(0.0);
    assert!((reflection - 1.0).abs() < 1.0e-6, "{reflection}");
}

#[test]
fn resetting_clears_the_state() {
    let mut comb = Comb::new(16);
    comb.set_length(4);
    comb.process(1.0);
    comb.reset();
    assert_eq!(comb.process(0.0), 0.0);

    let mut allpass = AllPass::new(16);
    allpass.set_length(4);
    allpass.process(1.0);
    allpass.reset();
    assert_eq!(allpass.process(0.0), 0.0);
}
