use pwe_core::{ParamSet, ParamSpec, ParamUnit};

use super::*;

fn tone(size: usize, bin: usize) -> Vec<f32> {
    (0..size)
        .map(|index| (std::f32::consts::TAU * bin as f32 * index as f32 / size as f32).cos())
        .collect()
}

fn magnitude_at(samples: &[f32], bin: usize) -> f32 {
    let size = samples.len();
    let fft = Fft::new(size);
    let mut re = samples.to_vec();
    let mut im = vec![0.0f32; size];
    fft.forward(&mut re, &mut im);
    (re[bin] * re[bin] + im[bin] * im[bin]).sqrt()
}

#[test]
fn a_biquad_and_the_fft_agree_that_a_low_pass_removes_a_high_tone() {
    let size = 256;
    let bin = 64;
    let input = tone(size, bin);
    let mut biquad = Biquad::new(Coefficients::design(
        FilterKind::LowPass,
        48_000.0,
        1_000.0,
        std::f32::consts::FRAC_1_SQRT_2,
        0.0,
    ));
    let filtered: Vec<f32> = input.iter().map(|sample| biquad.process(*sample)).collect();
    assert!(
        magnitude_at(&filtered, bin) < magnitude_at(&input, bin) * 0.2,
        "the low pass did not attenuate the tone"
    );
}

#[test]
fn a_knob_reads_the_parameter_it_names() {
    let params = ParamSet::new(vec![
        ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
            .range(-12.0, 12.0, 3.0)
            .build(),
    ]);
    let knob = Knob::new(&params, "gain", 0.0);
    assert_eq!(knob.value(), 3.0);
    params.set_f32("gain", -4.0).unwrap();
    assert_eq!(knob.value(), -4.0);
}
