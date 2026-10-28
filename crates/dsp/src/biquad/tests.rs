use super::*;
use crate::util::linear_to_db;

const RATE: f32 = 48_000.0;

#[test]
fn identity_passes_every_frequency_unchanged() {
    for frequency in [20.0_f32, 1_000.0, 10_000.0] {
        assert!((BiquadCoefficients::IDENTITY.magnitude_at(frequency, RATE) - 1.0).abs() < 1e-6);
    }
}

#[test]
fn peaking_filter_has_the_requested_gain_at_its_centre() {
    for gain in [6.0_f32, -6.0, 12.0, -12.0] {
        let section = BiquadCoefficients::peaking(1_000.0, RATE, 1.0, gain);
        let measured = linear_to_db(section.magnitude_at(1_000.0, RATE));
        assert!(
            (measured - gain).abs() < 0.05,
            "expected {gain} dB at the centre, measured {measured}"
        );
    }
}

#[test]
fn a_peaking_filter_returns_to_unity_away_from_its_centre() {
    let section = BiquadCoefficients::peaking(1_000.0, RATE, 1.0, 12.0);
    assert!(linear_to_db(section.magnitude_at(20.0, RATE)).abs() < 0.5);
    assert!(linear_to_db(section.magnitude_at(18_000.0, RATE)).abs() < 0.5);
}

#[test]
fn shelves_reach_their_gain_on_the_correct_side_of_the_corner() {
    let low = BiquadCoefficients::low_shelf(200.0, RATE, 1.0, 6.0);
    assert!((linear_to_db(low.magnitude_at(20.0, RATE)) - 6.0).abs() < 0.2);
    assert!(linear_to_db(low.magnitude_at(8_000.0, RATE)).abs() < 0.2);

    let high = BiquadCoefficients::high_shelf(4_000.0, RATE, 1.0, -6.0);
    assert!((linear_to_db(high.magnitude_at(16_000.0, RATE)) + 6.0).abs() < 0.3);
    assert!(linear_to_db(high.magnitude_at(100.0, RATE)).abs() < 0.2);
}

#[test]
fn a_flat_shelf_is_transparent() {
    let low = BiquadCoefficients::low_shelf(200.0, RATE, 1.0, 0.0);
    for frequency in [20.0_f32, 1_000.0, 10_000.0] {
        assert!(linear_to_db(low.magnitude_at(frequency, RATE)).abs() < 1e-3);
    }
}

#[test]
fn low_pass_passes_low_frequencies_and_rejects_high_ones() {
    let section = BiquadCoefficients::low_pass(1_000.0, RATE, 0.707);
    assert!(linear_to_db(section.magnitude_at(50.0, RATE)).abs() < 0.1);
    assert!((linear_to_db(section.magnitude_at(1_000.0, RATE)) + 3.0).abs() < 0.2);
    assert!(linear_to_db(section.magnitude_at(18_000.0, RATE)) < -40.0);
}

#[test]
fn high_pass_passes_high_frequencies_and_rejects_low_ones() {
    let section = BiquadCoefficients::high_pass(1_000.0, RATE, 0.707);
    assert!(linear_to_db(section.magnitude_at(18_000.0, RATE)).abs() < 0.1);
    assert!((linear_to_db(section.magnitude_at(1_000.0, RATE)) + 3.0).abs() < 0.2);
    assert!(linear_to_db(section.magnitude_at(20.0, RATE)) < -30.0);
}

#[test]
fn a_notch_rejects_exactly_its_centre_frequency() {
    let section = BiquadCoefficients::notch(1_000.0, RATE, 2.0);
    assert!(linear_to_db(section.magnitude_at(1_000.0, RATE)) < -60.0);
    assert!(linear_to_db(section.magnitude_at(8_000.0, RATE)).abs() < 0.5);
}

#[test]
fn a_band_pass_is_unity_at_its_centre() {
    let section = BiquadCoefficients::band_pass(1_000.0, RATE, 1.0);
    assert!(linear_to_db(section.magnitude_at(1_000.0, RATE)).abs() < 1e-3);
    assert!(linear_to_db(section.magnitude_at(50.0, RATE)) < -20.0);
}

#[test]
fn an_all_pass_is_flat_in_magnitude() {
    let section = BiquadCoefficients::all_pass(1_000.0, RATE, 1.0);
    for frequency in [20.0_f32, 200.0, 1_000.0, 5_000.0, 18_000.0] {
        assert!(linear_to_db(section.magnitude_at(frequency, RATE)).abs() < 1e-3);
    }
}

#[test]
fn every_design_produces_a_stable_section() {
    for frequency in [20.0_f32, 200.0, 1_000.0, 10_000.0, 20_000.0] {
        for q in [0.1_f32, 0.707, 2.0, 10.0] {
            for gain in [-24.0_f32, 0.0, 24.0] {
                for section in [
                    BiquadCoefficients::peaking(frequency, RATE, q, gain),
                    BiquadCoefficients::low_shelf(frequency, RATE, q, gain),
                    BiquadCoefficients::high_shelf(frequency, RATE, q, gain),
                    BiquadCoefficients::low_pass(frequency, RATE, q),
                    BiquadCoefficients::high_pass(frequency, RATE, q),
                    BiquadCoefficients::band_pass(frequency, RATE, q),
                    BiquadCoefficients::notch(frequency, RATE, q),
                    BiquadCoefficients::all_pass(frequency, RATE, q),
                ] {
                    assert!(
                        section.is_stable(),
                        "unstable section: f={frequency} q={q} gain={gain}"
                    );
                }
            }
        }
    }
}

#[test]
fn degenerate_inputs_produce_a_usable_section() {
    for section in [
        BiquadCoefficients::peaking(f32::NAN, RATE, 1.0, 6.0),
        BiquadCoefficients::peaking(1_000.0, 0.0, 1.0, 6.0),
        BiquadCoefficients::peaking(1_000.0, RATE, 0.0, f32::INFINITY),
        BiquadCoefficients::peaking(-500.0, RATE, -1.0, f32::NAN),
    ] {
        assert!(section.is_stable());
        assert!(section.b0.is_finite() && section.a1.is_finite());
    }
}

#[test]
fn state_processes_an_impulse_into_the_expected_response() {
    let section = BiquadCoefficients::peaking(1_000.0, RATE, 1.0, 6.0);
    let mut state = BiquadState::new();
    let first = state.process(&section, 1.0);
    assert!((first - section.b0).abs() < 1e-6);
}

#[test]
fn reset_clears_the_delay_memory() {
    let section = BiquadCoefficients::peaking(500.0, RATE, 2.0, 12.0);
    let mut state = BiquadState::new();
    for _ in 0..64 {
        state.process(&section, 1.0);
    }
    state.reset();
    let fresh = BiquadState::new();
    assert_eq!(state, fresh);
}

#[test]
fn a_filter_fed_silence_decays_to_silence_and_flushes() {
    let section = BiquadCoefficients::peaking(100.0, RATE, 4.0, 20.0);
    let mut state = BiquadState::new();
    for _ in 0..512 {
        state.process(&section, 1.0);
    }
    let mut last = 0.0;
    for _ in 0..200_000 {
        last = state.process(&section, 0.0);
    }
    assert!(
        last.abs() < 1e-9,
        "filter did not decay, last sample {last}"
    );
    state.sanitise();
    assert_eq!(state, BiquadState::new());
}

#[test]
fn sanitise_recovers_a_state_poisoned_by_a_non_finite_sample() {
    let section = BiquadCoefficients::peaking(1_000.0, RATE, 1.0, 6.0);
    let mut state = BiquadState::new();
    state.process(&section, f32::NAN);
    assert!(!state.is_healthy());

    state.sanitise();
    assert!(state.is_healthy());
    assert!(state.process(&section, 0.5).is_finite());
}

#[test]
fn cascading_two_sections_adds_their_gains_in_decibels() {
    let first = BiquadCoefficients::peaking(1_000.0, RATE, 1.0, 6.0);
    let second = BiquadCoefficients::peaking(1_000.0, RATE, 1.0, 6.0);
    let combined = linear_to_db(first.magnitude_at(1_000.0, RATE))
        + linear_to_db(second.magnitude_at(1_000.0, RATE));
    assert!((combined - 12.0).abs() < 0.1);
}
