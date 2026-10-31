use super::*;

#[test]
fn a_delta_impulse_passes_through_one_block_late() {
    let mut impulse = vec![0.0; BLOCK];
    impulse[0] = 1.0;
    let mut filter = Partitioned::new(BLOCK, &impulse);
    let output: Vec<f32> = (0..BLOCK * 2)
        .map(|index| filter.feed(f32::from(index == 0)))
        .collect();
    assert!(output[..BLOCK].iter().all(|value| value.abs() < 1.0e-6));
    assert!((output[BLOCK] - 1.0).abs() < 1.0e-5, "{}", output[BLOCK]);
    assert!(output[BLOCK + 1..].iter().all(|value| value.abs() < 1.0e-6));
}

#[test]
fn the_output_matches_direct_convolution() {
    let impulse: Vec<f32> = (0..37)
        .map(|index| ((index as f32 * 0.37).sin()) * (-(index as f32) * 0.05).exp())
        .collect();
    let input: Vec<f32> = (0..BLOCK * 3)
        .map(|index| ((index as f32 * 0.11).cos()) * 0.4)
        .collect();
    let mut filter = Partitioned::new(BLOCK, &impulse);
    let output: Vec<f32> = input.iter().map(|sample| filter.feed(*sample)).collect();
    for index in BLOCK..output.len() {
        let mut expected = 0.0f32;
        for (tap, coefficient) in impulse.iter().enumerate() {
            if index >= BLOCK + tap {
                expected += input[index - BLOCK - tap] * coefficient;
            }
        }
        assert!(
            (output[index] - expected).abs() < 1.0e-4,
            "sample {index}: {} against {expected}",
            output[index]
        );
    }
}

#[test]
fn reset_clears_the_history() {
    let impulse = vec![0.5; BLOCK];
    let mut filter = Partitioned::new(BLOCK, &impulse);
    for _ in 0..BLOCK * 2 {
        filter.feed(1.0);
    }
    filter.reset();
    let output: Vec<f32> = (0..BLOCK).map(|_| filter.feed(0.0)).collect();
    assert!(output.iter().all(|value| value.abs() < 1.0e-6));
}

#[test]
fn a_long_impulse_is_split_into_partitions() {
    let mut impulse = vec![0.0; BLOCK * 3 + 11];
    impulse[0] = 1.0;
    impulse[BLOCK * 2 + 5] = 0.25;
    let mut filter = Partitioned::new(BLOCK, &impulse);
    let output: Vec<f32> = (0..BLOCK * 5)
        .map(|index| filter.feed(f32::from(index == 0)))
        .collect();
    assert!((output[BLOCK] - 1.0).abs() < 1.0e-5);
    assert!((output[BLOCK * 3 + 5] - 0.25).abs() < 1.0e-5);
}
