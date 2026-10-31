use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

pub use pwe_dsp::spectrum::{BANDS, FFT_SIZES, FLOOR_DB, WINDOWS, band_centre};

#[derive(Debug)]
pub struct Spectrum {
    bands: [AtomicU64; BANDS],
    fft: AtomicU8,
    window: AtomicU8,
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
            fft: AtomicU8::new(2),
            window: AtomicU8::new(0),
        }
    }

    pub fn set_options(&self, fft: u8, window: u8) {
        self.fft
            .store(fft.min(FFT_SIZES.len() as u8 - 1), Ordering::Relaxed);
        self.window
            .store(window.min(WINDOWS.len() as u8 - 1), Ordering::Relaxed);
    }

    pub fn options(&self) -> (u8, u8) {
        (
            self.fft.load(Ordering::Relaxed),
            self.window.load(Ordering::Relaxed),
        )
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
