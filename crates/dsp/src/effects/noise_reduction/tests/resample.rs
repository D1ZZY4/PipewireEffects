use super::resample::LinearResampler;

const MAX_OUTPUT: usize = 8;

#[test]
fn a_constant_survives_both_directions() {
    let mut up = LinearResampler::new(44_100, 48_000);
    let mut down = LinearResampler::new(48_000, 44_100);
    let mut produced = 0usize;
    for _ in 0..44_100 {
        let mut upsampled = [0.0f32; MAX_OUTPUT];
        let count = up.push(0.5, &mut upsampled);
        for sample in upsampled.iter().take(count) {
            let mut downsampled = [0.0f32; MAX_OUTPUT];
            let emitted = down.push(*sample, &mut downsampled);
            for value in downsampled.iter().take(emitted) {
                assert!((value - 0.5).abs() < 1.0e-6);
                produced += 1;
            }
        }
    }
    assert!((produced as isize - 44_100).abs() <= 4, "{produced}");
}

#[test]
fn an_upsampling_ratio_produces_the_expected_count() {
    let mut resampler = LinearResampler::new(44_100, 48_000);
    let mut produced = 0usize;
    let mut out = [0.0f32; MAX_OUTPUT];
    for index in 0..44_100 {
        produced += resampler.push(index as f32, &mut out);
    }
    assert!((produced as isize - 48_000).abs() <= 2, "{produced}");
}

#[test]
fn a_ramp_is_interpolated_linearly() {
    let mut resampler = LinearResampler::new(24_000, 48_000);
    let mut out = [0.0f32; MAX_OUTPUT];
    let mut values = Vec::new();
    for index in 0..10 {
        let count = resampler.push(index as f32, &mut out);
        values.extend_from_slice(&out[..count]);
    }
    for (position, value) in values.iter().enumerate() {
        let wanted = position as f32 * 0.5;
        assert!((value - wanted).abs() < 1.0e-5, "{value} against {wanted}");
    }
}
