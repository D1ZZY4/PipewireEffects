use std::sync::atomic::{AtomicU64, Ordering};

pub use pwe_dsp::spectrum::{BANDS, FLOOR_DB, band_centre};

#[derive(Debug)]
pub struct Spectrum {
    bands: [AtomicU64; BANDS],
}

impl Default for Spectrum {
    fn default() -> Self {
        Self::new()
    }
}

impl Spectrum {
    pub fn new() -> Self {
        Self {
            bands: std::array::from_fn(|_| AtomicU64::new(floor_bits())),
        }
    }

    pub fn publish(&self, values: &[f32; BANDS]) {
        for (slot, value) in self.bands.iter().zip(values) {
            slot.store(u64::from(value.to_bits()), Ordering::Relaxed);
        }
    }

    pub fn clear(&self) {
        self.publish(&[FLOOR_DB; BANDS]);
    }

    pub fn snapshot(&self) -> [f32; BANDS] {
        std::array::from_fn(
            |index| f32::from_bits(self.bands[index].load(Ordering::Relaxed) as u32),
        )
    }
}

fn floor_bits() -> u64 {
    u64::from(FLOOR_DB.to_bits())
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
