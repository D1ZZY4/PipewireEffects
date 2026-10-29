use std::f32::consts::TAU;

use pwe_core::i18n::mark;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    AllPass,
    Peaking,
    LowShelf,
    HighShelf,
}

impl FilterKind {
    pub const ALL: [FilterKind; 8] = [
        FilterKind::LowPass,
        FilterKind::HighPass,
        FilterKind::BandPass,
        FilterKind::Notch,
        FilterKind::AllPass,
        FilterKind::Peaking,
        FilterKind::LowShelf,
        FilterKind::HighShelf,
    ];

    pub const LABELS: [&'static str; 8] = [
        mark("Low pass"),
        mark("High pass"),
        mark("Band pass"),
        mark("Notch"),
        mark("All pass"),
        mark("Peak"),
        mark("Low shelf"),
        mark("High shelf"),
    ];

    pub fn from_index(index: usize) -> FilterKind {
        *Self::ALL.get(index).unwrap_or(&FilterKind::LowPass)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for Coefficients {
    fn default() -> Self {
        Self::identity()
    }
}

impl Coefficients {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }

    pub fn design(kind: FilterKind, rate: f32, frequency: f32, q: f32, gain_db: f32) -> Self {
        let nyquist = 0.5 * rate;
        let frequency = frequency.clamp(1.0, (nyquist * 0.999).max(1.0));
        let q = q.clamp(0.05, 40.0);
        let omega = TAU * frequency / rate;
        let sin = omega.sin();
        let cos = omega.cos();
        let alpha = sin / (2.0 * q);
        let amplitude = 10.0f32.powf(gain_db / 40.0);
        let raw = match kind {
            FilterKind::LowPass => [
                (1.0 - cos) * 0.5,
                1.0 - cos,
                (1.0 - cos) * 0.5,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ],
            FilterKind::HighPass => [
                (1.0 + cos) * 0.5,
                -(1.0 + cos),
                (1.0 + cos) * 0.5,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ],
            FilterKind::BandPass => [alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha],
            FilterKind::Notch => [1.0, -2.0 * cos, 1.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha],
            FilterKind::AllPass => [
                1.0 - alpha,
                -2.0 * cos,
                1.0 + alpha,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ],
            FilterKind::Peaking => [
                1.0 + alpha * amplitude,
                -2.0 * cos,
                1.0 - alpha * amplitude,
                1.0 + alpha / amplitude,
                -2.0 * cos,
                1.0 - alpha / amplitude,
            ],
            FilterKind::LowShelf => {
                let beta = 2.0 * amplitude.sqrt() * alpha;
                [
                    amplitude * ((amplitude + 1.0) - (amplitude - 1.0) * cos + beta),
                    2.0 * amplitude * ((amplitude - 1.0) - (amplitude + 1.0) * cos),
                    amplitude * ((amplitude + 1.0) - (amplitude - 1.0) * cos - beta),
                    (amplitude + 1.0) + (amplitude - 1.0) * cos + beta,
                    -2.0 * ((amplitude - 1.0) + (amplitude + 1.0) * cos),
                    (amplitude + 1.0) + (amplitude - 1.0) * cos - beta,
                ]
            }
            FilterKind::HighShelf => {
                let beta = 2.0 * amplitude.sqrt() * alpha;
                [
                    amplitude * ((amplitude + 1.0) + (amplitude - 1.0) * cos + beta),
                    -2.0 * amplitude * ((amplitude - 1.0) + (amplitude + 1.0) * cos),
                    amplitude * ((amplitude + 1.0) + (amplitude - 1.0) * cos - beta),
                    (amplitude + 1.0) - (amplitude - 1.0) * cos + beta,
                    2.0 * ((amplitude - 1.0) - (amplitude + 1.0) * cos),
                    (amplitude + 1.0) - (amplitude - 1.0) * cos - beta,
                ]
            }
        };
        Self::normalised(raw)
    }

    pub fn magnitude_at(&self, rate: f32, frequency: f32) -> f32 {
        let omega = TAU * frequency.clamp(0.0, rate * 0.5) / rate;
        let (sin1, cos1) = omega.sin_cos();
        let (sin2, cos2) = (2.0 * omega).sin_cos();
        let numerator_real = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let numerator_imag = -self.b1 * sin1 - self.b2 * sin2;
        let denominator_real = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let denominator_imag = -self.a1 * sin1 - self.a2 * sin2;
        let numerator = (numerator_real * numerator_real + numerator_imag * numerator_imag).sqrt();
        let denominator =
            (denominator_real * denominator_real + denominator_imag * denominator_imag).sqrt();
        if denominator <= f32::MIN_POSITIVE {
            0.0
        } else {
            numerator / denominator
        }
    }

    pub fn is_stable(&self) -> bool {
        self.a2.abs() < 1.0 && self.a1.abs() < 1.0 + self.a2
    }

    fn normalised(raw: [f32; 6]) -> Self {
        let [b0, b1, b2, a0, a1, a2] = raw;
        if a0.abs() <= f32::MIN_POSITIVE {
            return Self::identity();
        }
        let inverse = 1.0 / a0;
        let coefficients = Self {
            b0: b0 * inverse,
            b1: b1 * inverse,
            b2: b2 * inverse,
            a1: a1 * inverse,
            a2: a2 * inverse,
        };
        if coefficients.b0.is_finite()
            && coefficients.b1.is_finite()
            && coefficients.b2.is_finite()
            && coefficients.is_stable()
        {
            coefficients
        } else {
            Self::identity()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    coefficients: Coefficients,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    pub fn new(coefficients: Coefficients) -> Self {
        Self {
            coefficients,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn set_coefficients(&mut self, coefficients: Coefficients) {
        self.coefficients = coefficients;
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let c = self.coefficients;
        let output =
            c.b0 * input + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = if output.is_finite() { output } else { 0.0 };
        self.y1
    }
}

#[cfg(test)]
mod tests {
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
}
