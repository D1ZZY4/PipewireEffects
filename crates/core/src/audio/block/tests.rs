use super::*;

#[test]
fn with_frames_clamps_to_the_backing_slice() {
    let mut buffer = [0.0_f32; 8];
    let block = AudioBlock::with_frames(&mut buffer, AudioFormat::new(48_000, 2), 99);
    assert_eq!(block.frames(), 4);
    assert_eq!(block.active_samples().len(), 8);
}

#[test]
fn new_derives_the_frame_count() {
    let mut buffer = [0.0_f32; 10];
    let block = AudioBlock::new(&mut buffer, AudioFormat::new(48_000, 2));
    assert_eq!(block.frames(), 5);
}

#[test]
fn peak_and_rms_use_the_active_region_only() {
    let mut buffer = [0.0_f32; 4];
    buffer[0] = 0.5;
    buffer[1] = -0.5;
    let block = AudioBlock::with_frames(&mut buffer, AudioFormat::new(48_000, 2), 1);
    assert!((block.peak() - 0.5).abs() < f32::EPSILON);
    assert!((block.rms() - 0.5).abs() < 1e-6);
}

#[test]
fn sanitise_removes_non_finite_samples() {
    let mut buffer = [f32::NAN, 1.0, f32::INFINITY, -1.0];
    let mut block = AudioBlock::new(&mut buffer, AudioFormat::new(48_000, 2));
    block.sanitise();
    assert_eq!(block.active_samples(), &[0.0, 1.0, 0.0, -1.0]);
}

#[test]
fn gain_scales_every_active_sample() {
    let mut buffer = [1.0_f32, -1.0, 1.0, -1.0];
    let mut block = AudioBlock::new(&mut buffer, AudioFormat::new(48_000, 2));
    block.apply_gain(0.5);
    assert_eq!(block.active_samples(), &[0.5, -0.5, 0.5, -0.5]);
}

#[test]
fn frames_iterate_in_channel_groups() {
    let mut buffer = [1.0_f32, 2.0, 3.0, 4.0];
    let block = AudioBlock::new(&mut buffer, AudioFormat::new(48_000, 2));
    let frames: Vec<&[f32]> = block.frames_iter().collect();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0], &[1.0, 2.0]);
}
