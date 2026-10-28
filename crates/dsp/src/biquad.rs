use crate::util::angular_frequency;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiquadCoefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for BiquadCoefficients {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl BiquadCoefficients {
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

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

    #[must_use]
    pub fn band_pass(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.01));

        Self::normalised(alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
    }

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

    #[must_use]
    pub fn magnitude_at(&self, frequency: f32, sample_rate: f32) -> f32 {
        let w0 = angular_frequency(frequency, sample_rate);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let (sin_2w0, cos_2w0) = (2.0 * w0).sin_cos();

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

    #[must_use]
    pub fn gain_db_at(&self, frequency: f32, sample_rate: f32) -> f32 {
        crate::util::linear_to_db(self.magnitude_at(frequency, sample_rate))
    }

    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.a2.abs() < 1.0 && self.a1.abs() < 1.0 + self.a2
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BiquadState {
    z1: f32,
    z2: f32,
}

impl BiquadState {
    #[must_use]
    pub const fn new() -> Self {
        Self { z1: 0.0, z2: 0.0 }
    }

    pub const fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, coefficients: &BiquadCoefficients, input: f32) -> f32 {
        let output = coefficients.b0.mul_add(input, self.z1);
        self.z1 = self.z2 + coefficients.b1.mul_add(input, -(coefficients.a1 * output));
        self.z2 = coefficients.b2.mul_add(input, -(coefficients.a2 * output));
        output
    }

    pub fn sanitise(&mut self) {
        self.z1 = crate::util::sanitise_value(self.z1);
        self.z2 = crate::util::sanitise_value(self.z2);
    }

    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.z1.is_finite() && self.z2.is_finite()
    }
}

#[cfg(test)]
mod tests;
