use super::*;
use crate::audio::{AudioBlock, AudioFormat};

fn stereo_block(samples: &mut [f32]) -> AudioBlock<'_> {
    AudioBlock::new(samples, AudioFormat::default_stereo())
}

#[test]
fn accumulation_reports_per_channel_peak_and_rms() {
    let mut samples = [0.5_f32, -0.25, 1.0, 0.0];
    let block = stereo_block(&mut samples);
    let mut meter = BlockMeter::new(2);
    meter.accumulate_block(&block);
    let shared = SharedMeters::new(2);
    meter.publish(&shared);

    let snapshot = shared.snapshot();
    assert_eq!(snapshot.channels(), 2);
    assert!((snapshot.peak_of(0) - 1.0).abs() < f32::EPSILON);
    assert!((snapshot.peak_of(1) - 0.25).abs() < f32::EPSILON);
    assert!((snapshot.rms_of(0) - 0.625_f32.sqrt()).abs() < 1e-6);
}

#[test]
fn publishing_clears_the_accumulators() {
    let mut samples = [1.0_f32, 1.0];
    let block = stereo_block(&mut samples);
    let mut meter = BlockMeter::new(2);
    meter.accumulate_block(&block);
    let shared = SharedMeters::new(2);
    meter.publish(&shared);
    meter.publish(&shared);
    assert!(!shared.snapshot().is_clipping());
    assert!(shared.snapshot().is_silent());
}

#[test]
fn clipping_is_detected_at_full_scale() {
    let mut samples = [1.0_f32, 0.0];
    let block = stereo_block(&mut samples);
    let mut meter = BlockMeter::new(2);
    meter.accumulate_block(&block);
    let shared = SharedMeters::new(2);
    meter.publish(&shared);
    assert!(shared.snapshot().is_clipping());
}

#[test]
fn mismatch_between_block_and_meter_channels_is_survivable() {
    let mut samples = [0.0_f32; 6];
    let block = AudioBlock::new(&mut samples, AudioFormat::new(48_000, 6));
    let mut meter = BlockMeter::new(2);
    meter.accumulate_block(&block);
    let shared = SharedMeters::new(2);
    meter.publish(&shared);
    assert!(shared.snapshot().is_silent());
}

#[test]
fn storing_fewer_values_than_channels_leaves_the_rest_alone() {
    let meters = SharedMeters::new(2);
    meters.store(&[0.9], &[0.4]);
    let snapshot = meters.snapshot();
    assert!((snapshot.peak_of(0) - 0.9).abs() < f32::EPSILON);
    assert!(snapshot.peak_of(1).abs() < f32::EPSILON);
}

#[test]
fn reset_zeroes_every_channel() {
    let meters = SharedMeters::new(4);
    meters.store(&[1.0; 4], &[1.0; 4]);
    meters.reset();
    assert!(meters.snapshot().is_silent());
}
