use super::*;

#[test]
fn a_constant_signal_keeps_its_level() {
    let mut resampler = Resampler2x::new();
    let mut value = 0.0;
    for _ in 0..64 {
        let pair = resampler.upsample(0.25);
        value = resampler.downsample(pair);
    }
    assert!((value - 0.25).abs() < 0.0025, "value {value}");
}

#[test]
fn silence_stays_silent() {
    let mut resampler = Resampler2x::new();
    for _ in 0..16 {
        let pair = resampler.upsample(0.0);
        assert_eq!(resampler.downsample(pair), 0.0);
    }
}

#[test]
fn reset_clears_the_history() {
    let mut resampler = Resampler2x::new();
    for _ in 0..8 {
        let pair = resampler.upsample(0.5);
        resampler.downsample(pair);
    }
    resampler.reset();
    let pair = resampler.upsample(0.0);
    assert_eq!(resampler.downsample(pair), 0.0);
}
