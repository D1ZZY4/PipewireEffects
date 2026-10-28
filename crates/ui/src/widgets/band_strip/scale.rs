pub const MIN_DB: f64 = -24.0;
pub const MAX_DB: f64 = 24.0;

pub fn position_of(db: f64) -> f64 {
    (MAX_DB - db.clamp(MIN_DB, MAX_DB)) / (MAX_DB - MIN_DB)
}

pub fn gain_at(y: f64, height: i32) -> f32 {
    let height = f64::from(height).max(1.0);
    let position = (y / height).clamp(0.0, 1.0);
    #[allow(clippy::cast_possible_truncation)]
    let value = (MAX_DB - position * (MAX_DB - MIN_DB)) as f32;
    (value * 10.0).round() / 10.0
}

pub fn format_frequency(hz: f32) -> String {
    if hz >= 1_000.0 {
        format!("{:.1}k", hz / 1_000.0)
    } else {
        format!("{hz:.0}")
    }
}

#[cfg(test)]
mod tests;
