//! Second-order IIR sections, designed with the RBJ audio EQ cookbook.
//!
//! A biquad is the workhorse of audio equalisation: two poles and two zeros are
//! enough to build peaking, shelving, high-pass, low-pass, notch, band-pass and
//! all-pass responses, and cascading them gives you any equaliser curve you
//! want. Every filter here is normalised so that `a0 == 1`.
//!
//! The state is deliberately separate from the coefficients. An equaliser needs
//! one coefficient set per band but one *state* per band **per channel**, and
//! keeping them apart makes that explicit instead of an array of tuples.
//!
//! Coefficients are computed with the [RBJ cookbook][rbj] formulas, which are
//! the de-facto standard and are documented in terms of the parameters a user
//! actually understands: centre frequency, Q, and gain in dB.
//!
//! [rbj]: https://www.w3.org/TR/audio-eq-cookbook/

use crate::util::angular_frequency;

/// The normalised coefficients of a second-order section.
///
/// The transfer function is
///
/// ```text
///        b0 + b1·z⁻¹ + b2·z⁻²
/// H(z) = ─────────────────────
///         1 + a1·z⁻¹ + a2·z⁻²
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiquadCoefficients {
    /// Feed-forward coefficient of `x[n]`.
    pub b0: f32,
    /// Feed-forward coefficient of `x[n-1]`.
    pub b1: f32,
    /// Feed-forward coefficient of `x[n-2]`.
    pub b2: f32,
    /// Feedback coefficient of `y[n-1]`.
    pub a1: f32,
    /// Feedback coefficient of `y[n-2]`.
    pub a2: f32,
}

impl Default for BiquadCoefficients {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl BiquadCoefficients {
    /// A pass-through section.
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Builds a section from raw coefficients, normalising by `a0`.
    ///
    /// Falls back to [`BiquadCoefficients::IDENTITY`] when the result would not
    /// be usable. Two conditions are checked, and both matter:
    ///
    /// * every coefficient must be finite, so that `NaN` or infinity from a
    ///   malformed parameter cannot poison the audio stream, and
    /// * `|a2|` must be strictly below one. `a2` is the product of the pole
    ///   positions, so this rejects a pole sitting on the unit circle. Every
    ///   *degenerate* design lands there — an invalid sample rate or a
    ///   non-finite frequency collapses the design to `a2 == 1` — and a
    ///   marginally stable section would ring forever rather than decay.
    ///   Rejecting it turns each of those cases into a harmless pass-through.
    ///
    /// Well-formed RBJ designs always satisfy both; see
    /// `every_design_produces_a_stable_section`.
    #[must_use]
    pub fn normalised(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        if !(a0.is_finite() && a1.is_finite() && a2.is_finite() && a0.abs() > f32::MIN_POSITIVE) {
            return Self::IDENTITY;
        }
        let section = Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        };
        if section.b0.is_finite()
            && section.b1.is_finite()
            && section.b2.is_finite()
            && section.a1.is_finite()
            && section.a2.is_finite()
            && section.a2.abs() < 1.0
        {
            section
        } else {
            Self::IDENTITY
        }
    }

    /// A peaking (bell) filter: `gain_db` at `frequency`, unity elsewhere.
    #[must_use]
    pub fn peaking(frequency: f32, sample_rate: f32, q: f32, gain_db: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.01));
        let a = 10.0_f32.powf(gain_db / 40.0);

        Self::normalised(
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        )
    }

    /// A low shelf: `gain_db` below `frequency`, unity above.
    ///
    /// `slope` controls how sharp the transition is. The RBJ formulation is
    /// only defined for `slope <= 1`, where `1` is the steepest shelf the
    /// second-order structure can produce; larger values would make the term
    /// under the square root negative, so the value is clamped. Passing a Q
    /// factor straight through — as
    /// [`ParametricEqEffect`](crate::effects::eq::ParametricEqEffect) does — is
    /// therefore safe.
    #[must_use]
    pub fn low_shelf(frequency: f32, sample_rate: f32, slope: f32, gain_db: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let a = 10.0_f32.powf(gain_db / 40.0);
        let slope = slope.clamp(1e-4, 1.0);
        let alpha = sin_w0 * 0.5 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).max(0.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        Self::normalised(
            a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
            a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
            (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
            (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
        )
    }

    /// A high shelf: `gain_db` above `frequency`, unity below.
    ///
    /// `slope` is clamped exactly as for [`BiquadCoefficients::low_shelf`].
    #[must_use]
    pub fn high_shelf(frequency: f32, sample_rate: f32, slope: f32, gain_db: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let a = 10.0_f32.powf(gain_db / 40.0);
        let slope = slope.clamp(1e-4, 1.0);
        let alpha = sin_w0 * 0.5 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).max(0.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        Self::normalised(
            a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
            a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
            (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
            (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
        )
    }

    /// A second-order low-pass with the given Q.
    #[must_use]
    pub fn low_pass(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.01));
        let one_minus_cos = 1.0 - cos_w0;

        Self::normalised(
            one_minus_cos * 0.5,
            one_minus_cos,
            one_minus_cos * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    /// A second-order high-pass with the given Q.
    #[must_use]
    pub fn high_pass(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.01));
        let one_plus_cos = 1.0 + cos_w0;

        Self::normalised(
            one_plus_cos * 0.5,
            -one_plus_cos,
            one_plus_cos * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    /// A band-pass with unity gain at the centre frequency.
    #[must_use]
    pub fn band_pass(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.01));

        Self::normalised(alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
    }

    /// A notch that rejects `frequency`.
    #[must_use]
    pub fn notch(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.01));

        Self::normalised(
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    /// An all-pass section: flat magnitude, frequency-dependent phase.
    #[must_use]
    pub fn all_pass(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.01));

        Self::normalised(
            1.0 - alpha,
            -2.0 * cos_w0,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    /// The magnitude response at `frequency`, as a linear multiplier.
    ///
    /// Evaluates `|H(e^{jw})|` directly, which is what the tests assert
    /// against; it is not used on the audio path.
    #[must_use]
    pub fn magnitude_at(&self, frequency: f32, sample_rate: f32) -> f32 {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let (sin_2w0, cos_2w0) = (2.0 * w0).sin_cos();

        // e^{-jw} and e^{-2jw}, split into real and imaginary parts.
        let (c1, s1) = (cos_w0, -sin_w0);
        let (c2, s2) = (cos_2w0, -sin_2w0);

        let numerator_real = self.b0 + self.b1 * c1 + self.b2 * c2;
        let numerator_imag = self.b1 * s1 + self.b2 * s2;
        let denominator_real = 1.0 + self.a1 * c1 + self.a2 * c2;
        let denominator_imag = self.a1 * s1 + self.a2 * s2;

        let numerator = numerator_real.hypot(numerator_imag);
        let denominator = denominator_real.hypot(denominator_imag);
        if denominator <= f32::MIN_POSITIVE {
            0.0
        } else {
            numerator / denominator
        }
    }

    /// The magnitude response in decibels.
    #[must_use]
    pub fn gain_db_at(&self, frequency: f32, sample_rate: f32) -> f32 {
        crate::util::linear_to_db(self.magnitude_at(frequency, sample_rate))
    }

    /// Whether both poles lie inside the unit circle.
    ///
    /// For `z² + a1·z + a2` the Jury stability test reduces to these two
    /// inequalities, which is all a second-order section needs.
    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.a2.abs() < 1.0 && self.a1.abs() < 1.0 + self.a2
    }
}

/// The delay memory of one second-order section.
///
/// Uses the transposed direct form II structure, which needs only two state
/// variables and has good numerical behaviour for the coefficient ranges an
/// equaliser produces.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BiquadState {
    z1: f32,
    z2: f32,
}

impl BiquadState {
    /// Creates a section with cleared memory.
    #[must_use]
    pub const fn new() -> Self {
        Self { z1: 0.0, z2: 0.0 }
    }

    /// Clears the delay memory. Always call this when audio restarts, or a
    /// burst of stale samples will be emitted.
    pub const fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Filters one sample.
    ///
    /// Transposed direct form II:
    ///
    /// ```text
    /// y[n]  = b0·x[n] + z1
    /// z1   ← b1·x[n] − a1·y[n] + z2
    /// z2   ← b2·x[n] − a2·y[n]
    /// ```
    #[inline]
    pub fn process(&mut self, coefficients: &BiquadCoefficients, input: f32) -> f32 {
        let output = coefficients.b0.mul_add(input, self.z1);
        self.z1 = self.z2 + coefficients.b1.mul_add(input, -(coefficients.a1 * output));
        self.z2 = coefficients.b2.mul_add(input, -(coefficients.a2 * output));
        output
    }

    /// Replaces subnormal and non-finite state with zero.
    ///
    /// Called once per block rather than per sample: denormals only ever appear
    /// when a filter has been fed silence for a while, and a `NaN` can only
    /// arrive from upstream. Sanitising at the block boundary keeps a branch
    /// out of the inner loop while bounding the damage from either to a single
    /// block.
    pub fn sanitise(&mut self) {
        self.z1 = crate::util::sanitise_value(self.z1);
        self.z2 = crate::util::sanitise_value(self.z2);
    }

    /// Whether the delay memory holds only finite, normal values.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.z1.is_finite() && self.z2.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::linear_to_db;

    const RATE: f32 = 48_000.0;

    #[test]
    fn identity_passes_every_frequency_unchanged() {
        for frequency in [20.0_f32, 1_000.0, 10_000.0] {
            assert!(
                (BiquadCoefficients::IDENTITY.magnitude_at(frequency, RATE) - 1.0).abs() < 1e-6
            );
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
        // A peaking filter at +6 dB should roughly double a DC-ish impulse
        // sum, so check the first output sample equals b0.
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
        // The filter produces finite output again on the very next block.
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
}
