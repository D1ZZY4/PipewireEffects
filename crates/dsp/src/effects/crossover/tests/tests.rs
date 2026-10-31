use super::*;
use std::f32::consts::TAU;

fn tone(frequency: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|index| (TAU * frequency * index as f32 / 48_000.0).sin())
        .collect()
}

fn reconstruct(crossover: &mut Crossover, input: &[f32], bands: usize) -> f32 {
    let mut peak = 0.0f32;
    for (index, sample) in input.iter().enumerate() {
        let mut sum = 0.0f32;
        for band in 0..bands {
            sum += crossover.run(0, band, *sample);
        }
        if index > 4_096 {
            peak = peak.max(sum.abs());
        }
    }
    peak
}

#[test]
fn the_bands_reconstruct_the_input_flat() {
    for frequency in [80.0f32, 300.0, 1_200.0, 6_000.0] {
        let mut crossover = Crossover::default();
        crossover.design(4, 48_000.0, &[120.0, 800.0, 5_000.0]);
        let peak = reconstruct(&mut crossover, &tone(frequency, 16_384), 4);
        assert!(
            (peak - 1.0).abs() < 0.06,
            "{frequency} Hz reconstructed to {peak}"
        );
    }
}

#[test]
fn eight_bands_reconstruct_within_half_a_decibel() {
    let splits = [80.0f32, 200.0, 500.0, 1_200.0, 3_000.0, 6_000.0, 12_000.0];
    for frequency in [80.0f32, 300.0, 1_000.0, 2_500.0, 6_000.0, 15_000.0] {
        let mut crossover = Crossover::default();
        crossover.design(8, 48_000.0, &splits);
        let peak = reconstruct(&mut crossover, &tone(frequency, 16_384), 8);
        assert!(
            (peak - 1.0).abs() < 0.06,
            "{frequency} Hz reconstructed to {peak}"
        );
    }
}

#[test]
fn a_well_separated_tone_dominates_its_band() {
    let mut crossover = Crossover::default();
    crossover.design(4, 48_000.0, &[120.0, 800.0, 5_000.0]);
    let input = tone(30.0, 16_384);
    let mut low = 0.0f32;
    let mut high = 0.0f32;
    for (index, sample) in input.iter().enumerate() {
        if index > 4_096 {
            low = low.max(crossover.run(0, 0, *sample).abs());
            for band in 1..4 {
                high = high.max(crossover.run(0, band, *sample).abs());
            }
        }
    }
    assert!(low > 0.9, "low band only reached {low}");
    assert!(high * 5.0 < low, "high bands leaked {high} against {low}");
}
