use std::f32::consts::PI;

pub const SILENCE_FLOOR_DB: f32 = -180.0;

#[must_use]
pub fn db_to_linear(db: f32) -> f32 {
    if db <= SILENCE_FLOOR_DB {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

#[must_use]
pub fn linear_to_db(linear: f32) -> f32 {
    if !linear.is_finite() || linear <= 0.0 {
        SILENCE_FLOOR_DB
    } else {
        (20.0 * linear.log10()).max(SILENCE_FLOOR_DB)
    }
}

#[must_use]
pub fn apply_db(linear: f32, db: f32) -> f32 {
    linear * db_to_linear(db)
}

#[must_use]
pub fn one_pole_coefficient(time_ms: f32, sample_rate: f32) -> f32 {
    let samples = (time_ms.max(0.0) * 0.001 * sample_rate).max(1.0);
    1.0 - (-1.0 / samples).exp()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smoother {
    value: f32,
    coefficient: f32,
}

impl Smoother {
    #[must_use]
    pub fn new(value: f32, time_ms: f32, sample_rate: f32) -> Self {
        let mut smoother = Self {
            value,
            coefficient: 1.0,
        };
        smoother.set_time_constant(time_ms, sample_rate);
        smoother
    }

    pub fn set_time_constant(&mut self, time_ms: f32, sample_rate: f32) {
        self.coefficient = one_pole_coefficient(time_ms, sample_rate);
    }

    #[must_use]
    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn reset(&mut self, value: f32) {
        self.value = value;
    }

    pub fn process(&mut self, target: f32) -> f32 {
        self.value += (target - self.value) * self.coefficient;
        self.value
    }

    pub fn advance(&mut self, target: f32, samples: usize) -> f32 {
        let retained = (1.0 - self.coefficient).powi(i32::try_from(samples).unwrap_or(i32::MAX));
        self.value = target + (self.value - target) * retained;
        self.value
    }

    #[must_use]
    pub fn is_settled(&self, target: f32, tolerance: f32) -> bool {
        (self.value - target).abs() <= tolerance
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    current: f32,
    target: f32,
    step: f32,
}

impl Ramp {
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.0,
        }
    }

    #[must_use]
    pub const fn target(&self) -> f32 {
        self.target
    }

    #[must_use]
    pub const fn current(&self) -> f32 {
        self.current
    }

    pub const fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.step == 0.0 && (self.current - self.target).abs() <= f32::EPSILON
    }

    pub const fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
        self.step = 0.0;
    }

    pub fn begin(&mut self, frames: usize) {
        if frames == 0 {
            self.current = self.target;
            self.step = 0.0;
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        let frames = frames as f32;
        self.step = (self.target - self.current) / frames;
    }

    pub fn next_value(&mut self) -> f32 {
        let value = self.current;
        if self.step != 0.0 {
            self.current += self.step;
        }
        value
    }

    pub fn finish(&mut self) {
        self.current = self.target;
        self.step = 0.0;
    }
}

#[must_use]
pub fn angular_frequency(frequency: f32, sample_rate: f32) -> f32 {
    if !frequency.is_finite() || sample_rate <= 0.0 {
        return 0.0;
    }
    let nyquist = sample_rate * 0.5;
    let clamped = frequency.clamp(1.0, (nyquist - 1.0).max(2.0));
    (2.0 * PI * clamped / sample_rate).clamp(1e-6, PI - 1e-6)
}

#[must_use]
pub fn sanitise_value(value: f32) -> f32 {
    if value.is_finite() && !value.is_subnormal() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
