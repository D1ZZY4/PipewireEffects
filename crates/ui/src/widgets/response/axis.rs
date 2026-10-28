pub const MIN_HZ: f64 = 20.0;
pub const MAX_HZ: f64 = 20_000.0;
pub const MIN_DB: f64 = -24.0;
pub const MAX_DB: f64 = 24.0;

pub fn frequency_position(hz: f64) -> f64 {
    let hz = hz.clamp(MIN_HZ, MAX_HZ);
    (hz.log10() - MIN_HZ.log10()) / (MAX_HZ.log10() - MIN_HZ.log10())
}

pub fn frequency_of(position: f64) -> f64 {
    let log = MIN_HZ.log10() + position * (MAX_HZ.log10() - MIN_HZ.log10());
    10f64.powf(log)
}

pub fn gain_position(db: f64) -> f64 {
    (MAX_DB - db.clamp(MIN_DB, MAX_DB)) / (MAX_DB - MIN_DB)
}

pub fn gain_of(position: f64) -> f64 {
    MAX_DB - position * (MAX_DB - MIN_DB)
}

#[cfg(test)]
mod tests;
