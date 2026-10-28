pub const MIN_GAIN: f32 = 1.0e-6;
pub const DENORMAL_THRESHOLD: f32 = 1.0e-25;

pub fn db_to_gain(db: f32) -> f32 {
    if db <= -144.0 {
        0.0
    } else {
        10.0f32.powf(db / 20.0)
    }
}

pub fn gain_to_db(gain: f32) -> f32 {
    if gain <= MIN_GAIN {
        -144.0
    } else {
        20.0 * gain.log10()
    }
}

pub fn flush_denormal(value: f32) -> f32 {
    if value.is_finite() && value.abs() > DENORMAL_THRESHOLD {
        value
    } else {
        0.0
    }
}

pub fn sanitize(value: f32) -> f32 {
    if value.is_finite() {
        flush_denormal(value)
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Smoothed {
    current: f32,
    target: f32,
    coefficient: f32,
}

impl Smoothed {
    pub fn new(initial: f32) -> Self {
        Self {
            current: initial,
            target: initial,
            coefficient: 1.0,
        }
    }

    pub fn set_time_constant(&mut self, ms: f32, rate: u32) {
        let seconds = (ms.max(0.01) / 1000.0).max(1.0e-6);
        self.coefficient = (1.0 - (-1.0 / (seconds * rate as f32)).exp()).clamp(0.0, 1.0);
    }

    pub fn set_target(&mut self, value: f32) {
        self.target = value;
    }

    pub fn snap(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    pub fn advance(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.coefficient;
        self.current = sanitize(self.current);
        self.current
    }

    pub fn current(&self) -> f32 {
        self.current
    }

    pub fn is_settled(&self) -> bool {
        (self.target - self.current).abs() < 1.0e-7
    }
}

pub fn interpolate(table: &[f32], index: f32) -> f32 {
    if table.is_empty() {
        return 0.0;
    }
    if table.len() == 1 {
        return table[0];
    }
    let position = index.clamp(0.0, (table.len() - 1) as f32);
    let base = position.floor();
    let lower = base as usize;
    let upper = (lower + 1).min(table.len() - 1);
    let fraction = position - base;
    table[lower] * (1.0 - fraction) + table[upper] * fraction
}

pub fn one_pole_alpha(ms: f32, rate: u32) -> f32 {
    let seconds = (ms.max(0.001) / 1000.0).max(1.0e-7);
    (1.0 - (-1.0 / (seconds * rate as f32)).exp()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_round_trip() {
        for db in [-60.0, -6.0, 0.0, 6.0, 24.0] {
            assert!((gain_to_db(db_to_gain(db)) - db).abs() < 1.0e-3);
        }
    }

    #[test]
    fn sanitize_flushes_non_finite() {
        assert_eq!(sanitize(f32::NAN), 0.0);
        assert_eq!(sanitize(f32::INFINITY), 0.0);
        assert_eq!(sanitize(1.0e-30), 0.0);
        assert_eq!(sanitize(0.25), 0.25);
    }

    #[test]
    fn smoothed_reaches_target() {
        let mut smoothed = Smoothed::new(0.0);
        smoothed.set_time_constant(1.0, 48_000);
        smoothed.set_target(1.0);
        for _ in 0..48_000 {
            smoothed.advance();
        }
        assert!((smoothed.current() - 1.0).abs() < 1.0e-3);
    }

    #[test]
    fn interpolate_matches_endpoints() {
        let table = [0.0, 10.0, 20.0];
        assert!((interpolate(&table, 0.0) - 0.0).abs() < 1.0e-6);
        assert!((interpolate(&table, 2.0) - 20.0).abs() < 1.0e-6);
        assert!((interpolate(&table, 1.5) - 15.0).abs() < 1.0e-6);
    }
}
