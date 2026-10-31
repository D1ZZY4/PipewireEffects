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
mod tests {
    use super::*;

    #[test]
    fn a_published_frame_is_returned_unchanged() {
        let spectrum = Spectrum::new();
        let mut values = [FLOOR_DB; BANDS];
        values[3] = -24.5;
        spectrum.publish(&values);
        let snapshot = spectrum.snapshot();
        assert_eq!(snapshot[3], -24.5);
        assert_eq!(snapshot[0], FLOOR_DB);
    }

    #[test]
    fn clearing_returns_every_band_to_the_floor() {
        let spectrum = Spectrum::new();
        let values = [0.0f32; BANDS];
        spectrum.publish(&values);
        spectrum.clear();
        assert!(spectrum.snapshot().iter().all(|value| *value <= FLOOR_DB));
    }
}
