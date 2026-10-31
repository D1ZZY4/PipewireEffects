const TAPS: usize = 15;
const CENTER: usize = 7;

pub(super) struct Resampler2x {
    up: [f32; TAPS],
    down: [f32; TAPS],
    coefficients: [f32; TAPS],
}

impl Resampler2x {
    pub(super) fn new() -> Self {
        let mut coefficients = [0.0f32; TAPS];
        let mut sum = 0.0;
        for (index, coefficient) in coefficients.iter_mut().enumerate() {
            let distance = index as f32 - CENTER as f32;
            let argument = 0.5 * std::f32::consts::PI * distance;
            let sinc = if distance.abs() < 1.0e-6 {
                1.0
            } else {
                argument.sin() / argument
            };
            let window =
                0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (TAPS as f32 - 1.0)).cos();
            *coefficient = 0.5 * sinc * window;
            sum += *coefficient;
        }
        for coefficient in &mut coefficients {
            *coefficient /= sum;
        }
        Self {
            up: [0.0; TAPS],
            down: [0.0; TAPS],
            coefficients,
        }
    }

    pub(super) fn reset(&mut self) {
        self.up.fill(0.0);
        self.down.fill(0.0);
    }

    pub(super) fn upsample(&mut self, sample: f32) -> [f32; 2] {
        self.up.copy_within(0..TAPS - 1, 1);
        self.up[0] = sample;
        let mut even = 0.0f32;
        let mut odd = 0.0f32;
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            if index % 2 == 0 {
                even += coefficient * self.up[index / 2];
            } else {
                odd += coefficient * self.up[(index - 1) / 2];
            }
        }
        [2.0 * even, 2.0 * odd]
    }

    pub(super) fn downsample(&mut self, pair: [f32; 2]) -> f32 {
        self.down.copy_within(0..TAPS - 2, 2);
        self.down[1] = pair[0];
        self.down[0] = pair[1];
        let mut value = 0.0f32;
        for (sample, coefficient) in self.down.iter().zip(self.coefficients) {
            value += sample * coefficient;
        }
        value
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
