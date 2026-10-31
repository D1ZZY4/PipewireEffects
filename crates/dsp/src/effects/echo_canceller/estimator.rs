const DECIMATION: usize = 32;
const WINDOW: usize = 96;
const MAX_DELAY_MS: f32 = 500.0;
const UPDATE_SAMPLES: usize = 32;
const MIN_CORRELATION: f32 = 0.1;

pub struct DelayEstimator {
    far: Vec<f32>,
    near: Vec<f32>,
    far_mask: usize,
    near_mask: usize,
    far_write: usize,
    near_write: usize,
    max_lag: usize,
    phase: usize,
    sum_far: f32,
    sum_near: f32,
    decimated: usize,
    delay: usize,
    correlation: f32,
    estimated: bool,
}

impl DelayEstimator {
    pub fn new(rate: u32) -> Self {
        let rate = rate.max(8_000);
        let max_lag = ((rate as f32 * MAX_DELAY_MS / 1000.0) as usize / DECIMATION).max(1);
        let far_len = (WINDOW + max_lag + 2).next_power_of_two();
        let near_len = WINDOW.next_power_of_two();
        Self {
            far: vec![0.0; far_len],
            near: vec![0.0; near_len],
            far_mask: far_len - 1,
            near_mask: near_len - 1,
            far_write: 0,
            near_write: 0,
            max_lag,
            phase: 0,
            sum_far: 0.0,
            sum_near: 0.0,
            decimated: 0,
            delay: 0,
            correlation: 0.0,
            estimated: false,
        }
    }

    pub fn reset(&mut self) {
        self.far.fill(0.0);
        self.near.fill(0.0);
        self.far_write = 0;
        self.near_write = 0;
        self.phase = 0;
        self.sum_far = 0.0;
        self.sum_near = 0.0;
        self.decimated = 0;
        self.delay = 0;
        self.correlation = 0.0;
        self.estimated = false;
    }

    pub fn push(&mut self, far: f32, near: f32) {
        self.sum_far += far;
        self.sum_near += near;
        self.phase += 1;
        if self.phase < DECIMATION {
            return;
        }
        let far = self.sum_far / DECIMATION as f32;
        let near = self.sum_near / DECIMATION as f32;
        self.sum_far = 0.0;
        self.sum_near = 0.0;
        self.phase = 0;
        self.far[self.far_write] = far;
        self.far_write = (self.far_write + 1) & self.far_mask;
        self.near[self.near_write] = near;
        self.near_write = (self.near_write + 1) & self.near_mask;
        self.decimated += 1;
        if self.decimated >= WINDOW + self.max_lag && self.decimated.is_multiple_of(UPDATE_SAMPLES)
        {
            self.estimate();
        }
    }

    fn estimate(&mut self) {
        let near_base = self.near_write + self.near_len() - WINDOW;
        let far_base = self.far_write + self.far_len() - WINDOW;
        let mut mean_near = 0.0f32;
        let mut near_energy = 0.0f32;
        for offset in 0..WINDOW {
            mean_near += self.near[(near_base + offset) & self.near_mask];
        }
        mean_near /= WINDOW as f32;
        for offset in 0..WINDOW {
            let value = self.near[(near_base + offset) & self.near_mask] - mean_near;
            near_energy += value * value;
        }
        if near_energy <= 1.0e-12 {
            self.correlation = 0.0;
            return;
        }
        let mut best_lag = 0usize;
        let mut best_correlation = -1.0f32;
        for lag in 0..=self.max_lag {
            let mut mean_far = 0.0f32;
            for offset in 0..WINDOW {
                mean_far += self.far[(far_base + offset - lag) & self.far_mask];
            }
            mean_far /= WINDOW as f32;
            let mut cross = 0.0f32;
            let mut far_energy = 0.0f32;
            for offset in 0..WINDOW {
                let far = self.far[(far_base + offset - lag) & self.far_mask] - mean_far;
                let near = self.near[(near_base + offset) & self.near_mask] - mean_near;
                cross += near * far;
                far_energy += far * far;
            }
            let denominator = (near_energy * far_energy).sqrt();
            if denominator <= 1.0e-12 {
                continue;
            }
            let correlation = cross / denominator;
            if correlation > best_correlation {
                best_correlation = correlation;
                best_lag = lag;
            }
        }
        if best_correlation > MIN_CORRELATION {
            self.delay = best_lag * DECIMATION;
            self.correlation = best_correlation;
            self.estimated = true;
        } else {
            self.correlation = best_correlation.max(0.0);
        }
    }

    fn near_len(&self) -> usize {
        self.near_mask + 1
    }

    fn far_len(&self) -> usize {
        self.far_mask + 1
    }

    pub fn delay_frames(&self) -> usize {
        self.delay
    }

    pub fn correlation(&self) -> f32 {
        self.correlation
    }

    pub fn has_estimate(&self) -> bool {
        self.estimated
    }
}
