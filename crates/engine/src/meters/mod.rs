use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::MAX_EFFECTS;

#[derive(Debug)]
pub struct Meters {
    input_peak: [AtomicU64; 2],
    input_rms: [AtomicU64; 2],
    output_peak: [AtomicU64; 2],
    output_rms: [AtomicU64; 2],
    loudness_momentary: AtomicU64,
    loudness_short_term: AtomicU64,
    loudness_integrated: AtomicU64,
    slot_peak_db: [AtomicU64; MAX_EFFECTS],
}

impl Default for Meters {
    fn default() -> Self {
        Self {
            input_peak: [AtomicU64::new(0), AtomicU64::new(0)],
            input_rms: [AtomicU64::new(0), AtomicU64::new(0)],
            output_peak: [AtomicU64::new(0), AtomicU64::new(0)],
            output_rms: [AtomicU64::new(0), AtomicU64::new(0)],
            loudness_momentary: AtomicU64::new(bits(f32::NEG_INFINITY)),
            loudness_short_term: AtomicU64::new(bits(f32::NEG_INFINITY)),
            loudness_integrated: AtomicU64::new(bits(f32::NEG_INFINITY)),
            slot_peak_db: std::array::from_fn(|_| AtomicU64::new(bits(f32::NEG_INFINITY))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterSnapshot {
    pub input_peak_db: [f32; 2],
    pub input_rms_db: [f32; 2],
    pub output_peak_db: [f32; 2],
    pub output_rms_db: [f32; 2],
    pub loudness_momentary_lufs: f32,
    pub loudness_short_term_lufs: f32,
    pub loudness_integrated_lufs: f32,
    pub slot_peak_db: [f32; MAX_EFFECTS],
}

impl Default for MeterSnapshot {
    fn default() -> Self {
        Self {
            input_peak_db: [f32::NEG_INFINITY; 2],
            input_rms_db: [f32::NEG_INFINITY; 2],
            output_peak_db: [f32::NEG_INFINITY; 2],
            output_rms_db: [f32::NEG_INFINITY; 2],
            loudness_momentary_lufs: f32::NEG_INFINITY,
            loudness_short_term_lufs: f32::NEG_INFINITY,
            loudness_integrated_lufs: f32::NEG_INFINITY,
            slot_peak_db: [f32::NEG_INFINITY; MAX_EFFECTS],
        }
    }
}

impl Meters {
    pub fn publish_input(&self, peak: [f32; 2], rms: [f32; 2]) {
        for channel in 0..2 {
            self.input_peak[channel].store(bits(peak[channel]), Ordering::Relaxed);
            self.input_rms[channel].store(bits(rms[channel]), Ordering::Relaxed);
        }
    }

    pub fn publish_output(&self, peak: [f32; 2], rms: [f32; 2]) {
        for channel in 0..2 {
            self.output_peak[channel].store(bits(peak[channel]), Ordering::Relaxed);
            self.output_rms[channel].store(bits(rms[channel]), Ordering::Relaxed);
        }
    }

    pub fn publish_loudness(
        &self,
        momentary: Option<f32>,
        short_term: Option<f32>,
        integrated: Option<f32>,
    ) {
        self.loudness_momentary.store(
            bits(momentary.unwrap_or(f32::NEG_INFINITY)),
            Ordering::Relaxed,
        );
        self.loudness_short_term.store(
            bits(short_term.unwrap_or(f32::NEG_INFINITY)),
            Ordering::Relaxed,
        );
        self.loudness_integrated.store(
            bits(integrated.unwrap_or(f32::NEG_INFINITY)),
            Ordering::Relaxed,
        );
    }

    pub fn snapshot(&self) -> MeterSnapshot {
        MeterSnapshot {
            input_peak_db: read_db(&self.input_peak),
            input_rms_db: read_db(&self.input_rms),
            output_peak_db: read_db(&self.output_peak),
            output_rms_db: read_db(&self.output_rms),
            loudness_momentary_lufs: read_scalar(&self.loudness_momentary),
            loudness_short_term_lufs: read_scalar(&self.loudness_short_term),
            loudness_integrated_lufs: read_scalar(&self.loudness_integrated),
            slot_peak_db: std::array::from_fn(|index| read_scalar(&self.slot_peak_db[index])),
        }
    }

    pub fn publish_slot_levels(&self, peaks: &[f32]) {
        for (index, slot) in self.slot_peak_db.iter().enumerate() {
            let peak = peaks.get(index).copied().unwrap_or(0.0);
            let db = if peak > 0.0 {
                pwe_core::math::gain_to_db(peak)
            } else {
                f32::NEG_INFINITY
            };
            slot.store(bits(db), Ordering::Relaxed);
        }
    }
}

fn bits(db: f32) -> u64 {
    u64::from(db.to_bits())
}

fn read_db(values: &[AtomicU64; 2]) -> [f32; 2] {
    [
        f32::from_bits(values[0].load(Ordering::Relaxed) as u32),
        f32::from_bits(values[1].load(Ordering::Relaxed) as u32),
    ]
}

fn read_scalar(value: &AtomicU64) -> f32 {
    f32::from_bits(value.load(Ordering::Relaxed) as u32)
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
