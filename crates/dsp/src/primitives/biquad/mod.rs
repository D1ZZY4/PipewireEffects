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
#[path = "tests/tests.rs"]
mod tests;
