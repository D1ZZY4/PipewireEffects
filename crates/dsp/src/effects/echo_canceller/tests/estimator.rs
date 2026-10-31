use super::estimator::DelayEstimator;

const RATE: u32 = 48_000;
const ECHO_DELAY: usize = 400;
const RING: usize = 2_048;

fn noise(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f32 / u32::MAX as f32) * 2.0 - 1.0
}

#[test]
fn the_estimator_finds_a_delayed_echo() {
    let mut estimator = DelayEstimator::new(RATE);
    let mut history = vec![0.0f32; RING];
    let mut cursor = 0usize;
    let mut state = 0x1234_5678u32;
    for _ in 0..96_000 {
        let far = noise(&mut state) * 0.3;
        history[cursor] = far;
        let near = history[(cursor + RING - ECHO_DELAY) % RING];
        cursor = (cursor + 1) % RING;
        estimator.push(far, near);
    }
    assert!(estimator.has_estimate(), "the estimator found no delay");
    let found = estimator.delay_frames() as isize;
    assert!(
        (found - ECHO_DELAY as isize).abs() <= 64,
        "the estimator found {found}"
    );
    assert!(estimator.correlation() > 0.3, "{}", estimator.correlation());
}

#[test]
fn silence_produces_no_delay_estimate() {
    let mut estimator = DelayEstimator::new(RATE);
    for _ in 0..96_000 {
        estimator.push(0.0, 0.0);
    }
    assert!(!estimator.has_estimate());
}
