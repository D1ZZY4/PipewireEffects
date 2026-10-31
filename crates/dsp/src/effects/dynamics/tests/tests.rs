use super::*;

#[test]
fn below_the_knee_there_is_no_reduction() {
    assert_eq!(soft_knee_reduction_db(-40.0, -18.0, 6.0, 4.0), 0.0);
}

#[test]
fn above_the_knee_matches_the_ratio() {
    let reduction = soft_knee_reduction_db(0.0, -18.0, 0.0, 4.0);
    let expected = -18.0 * 0.75;
    assert!((reduction - expected).abs() < 1.0e-4, "{reduction}");
}

#[test]
fn expansion_leaves_the_loud_half_alone() {
    assert_eq!(soft_knee_expansion_db(-6.0, -18.0, 6.0, 4.0), 0.0);
    assert_eq!(soft_knee_expansion_db(-18.0, -18.0, 0.0, 4.0), 0.0);
}

#[test]
fn expansion_matches_the_ratio_below_the_threshold() {
    let gain = soft_knee_expansion_db(-30.0, -18.0, 0.0, 4.0);
    assert!((gain + 36.0).abs() < 1.0e-4, "{gain}");
}

#[test]
fn the_expansion_knee_is_continuous() {
    let threshold = -18.0;
    let knee = 6.0;
    let ratio = 4.0;
    let below = soft_knee_expansion_db(threshold - knee * 0.5 - 1.0e-4, threshold, knee, ratio);
    let above = soft_knee_expansion_db(threshold - knee * 0.5 + 1.0e-4, threshold, knee, ratio);
    assert!((below - above).abs() < 1.0e-3, "{below} {above}");
    assert!((soft_knee_expansion_db(threshold, threshold, knee, ratio) + 2.25).abs() < 1.0e-2);
}

#[test]
fn the_knee_is_continuous() {
    let threshold = -18.0;
    let knee = 6.0;
    let ratio = 4.0;
    let below = soft_knee_reduction_db(threshold - knee * 0.5 - 1.0e-4, threshold, knee, ratio);
    let above = soft_knee_reduction_db(threshold + knee * 0.5 + 1.0e-4, threshold, knee, ratio);
    assert!(below.abs() < 1.0e-3);
    assert!((above - (-knee * 0.5 * 0.75)).abs() < 1.0e-2);
}

#[test]
fn ballistics_reach_their_targets() {
    let (attack, release) = ballistics(10.0, 100.0, 48_000);
    assert!(attack > release);
    let mut envelope = 0.0f32;
    for _ in 0..48_000 {
        let target = -6.0;
        let coefficient = if target < envelope { attack } else { release };
        envelope += (target - envelope) * coefficient;
    }
    assert!((envelope + 6.0).abs() < 0.01);
}

#[test]
fn the_detector_follows_a_steady_level() {
    let mut buffer = AudioBuffer::new(2, 64);
    buffer.configure(2, 64).unwrap();
    for channel in 0..2 {
        buffer.channel_mut(channel).fill(0.5);
    }
    let mut detector = Detector::default();
    detector.set_ballistics(1.0, 50.0, 48_000);
    let mut level = 0.0;
    for _ in 0..200 {
        for frame in 0..64 {
            let sample = buffer.channel(0)[frame];
            level = detector.track_measure(sample, sample * sample, false);
        }
    }
    assert!((level - 0.5).abs() < 1.0e-3, "{level}");
}

#[test]
fn the_mix_blend_crosses_between_dry_and_wet() {
    assert!((blend(0.0, 1.0, 0.25) - 0.25).abs() < 1.0e-6);
    assert!((blend(0.4, 0.4, 1.0) - 0.4).abs() < 1.0e-6);
}
