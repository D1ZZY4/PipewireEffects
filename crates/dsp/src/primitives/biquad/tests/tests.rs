use super::*;

#[test]
fn peaking_filter_hits_its_target_gain() {
    let coefficients = Coefficients::design(FilterKind::Peaking, 48_000.0, 1_000.0, 1.0, 6.0);
    let magnitude = coefficients.magnitude_at(48_000.0, 1_000.0);
    assert!(
        (20.0 * magnitude.log10() - 6.0).abs() < 0.05,
        "{}",
        20.0 * magnitude.log10()
    );
}

#[test]
fn peaking_filter_is_transparent_far_away() {
    let coefficients = Coefficients::design(FilterKind::Peaking, 48_000.0, 1_000.0, 1.0, 6.0);
    assert!(20.0 * coefficients.magnitude_at(48_000.0, 20.0).log10() < 0.2);
    assert!(20.0 * coefficients.magnitude_at(48_000.0, 18_000.0).log10() < 0.2);
}

#[test]
fn low_pass_reaches_half_power_at_cutoff() {
    let coefficients = Coefficients::design(
        FilterKind::LowPass,
        48_000.0,
        1_000.0,
        std::f32::consts::FRAC_1_SQRT_2,
        0.0,
    );
    let magnitude = coefficients.magnitude_at(48_000.0, 1_000.0);
    assert!((20.0 * magnitude.log10() + 3.01).abs() < 0.2);
}

#[test]
fn every_design_is_stable_across_a_sweep() {
    for kind in FilterKind::ALL {
        for gain in [-24.0f32, -6.0, 0.0, 6.0, 24.0] {
            for q in [0.1f32, 0.707, 2.0, 10.0] {
                for frequency in [20.0f32, 200.0, 2_000.0, 12_000.0, 20_000.0] {
                    let coefficients = Coefficients::design(kind, 48_000.0, frequency, q, gain);
                    assert!(coefficients.is_stable(), "{kind:?} {frequency} {q} {gain}");
                    assert!(coefficients.b0.is_finite());
                }
            }
        }
    }
}

#[test]
fn impulse_response_of_a_peaking_filter_is_finite() {
    let mut biquad = Biquad::new(Coefficients::design(
        FilterKind::Peaking,
        48_000.0,
        1_000.0,
        4.0,
        18.0,
    ));
    let mut energy = 0.0f32;
    for index in 0..48_000 {
        let input = if index == 0 { 1.0 } else { 0.0 };
        energy += biquad.process(input).abs();
    }
    assert!(energy.is_finite() && energy < 100.0, "{energy}");
}
