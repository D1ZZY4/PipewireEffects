use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct StreamStats {
    capture_frames: AtomicU64,
    processed_frames: AtomicU64,
    underrun_samples: AtomicU64,
    overrun_samples: AtomicU64,
    retired_drops: AtomicU64,
    trim_events: AtomicU64,
    trimmed_samples: AtomicU64,
    latency_samples: AtomicU64,
}

impl StreamStats {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn add_capture_frames(&self, frames: u64) {
        self.capture_frames.fetch_add(frames, Ordering::Relaxed);
    }

    pub fn add_processed_frames(&self, frames: u64) {
        self.processed_frames.fetch_add(frames, Ordering::Relaxed);
    }

    pub fn add_underrun_samples(&self, samples: u64) {
        self.underrun_samples.fetch_add(samples, Ordering::Relaxed);
    }

    pub fn add_overrun_samples(&self, samples: u64) {
        self.overrun_samples.fetch_add(samples, Ordering::Relaxed);
    }

    pub fn add_retired_drop(&self) {
        self.retired_drops.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_trim(&self, samples: u64) {
        self.trim_events.fetch_add(1, Ordering::Relaxed);
        self.trimmed_samples.fetch_add(samples, Ordering::Relaxed);
    }

    pub fn set_latency_samples(&self, samples: u64) {
        self.latency_samples.store(samples, Ordering::Relaxed);
    }

    #[must_use]
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            capture_frames: self.capture_frames.load(Ordering::Relaxed),
            processed_frames: self.processed_frames.load(Ordering::Relaxed),
            underrun_samples: self.underrun_samples.load(Ordering::Relaxed),
            overrun_samples: self.overrun_samples.load(Ordering::Relaxed),
            retired_drops: self.retired_drops.load(Ordering::Relaxed),
            trim_events: self.trim_events.load(Ordering::Relaxed),
            trimmed_samples: self.trimmed_samples.load(Ordering::Relaxed),
            latency_samples: self.latency_samples.load(Ordering::Relaxed),
        }
    }

    pub fn reset_problems(&self) {
        self.underrun_samples.store(0, Ordering::Relaxed);
        self.overrun_samples.store(0, Ordering::Relaxed);
        self.retired_drops.store(0, Ordering::Relaxed);
        self.trim_events.store(0, Ordering::Relaxed);
        self.trimmed_samples.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub capture_frames: u64,
    pub processed_frames: u64,
    pub underrun_samples: u64,
    pub overrun_samples: u64,
    pub retired_drops: u64,
    pub trim_events: u64,
    pub trimmed_samples: u64,
    pub latency_samples: u64,
}

impl StatsSnapshot {
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.capture_frames > 0 && self.processed_frames > 0
    }

    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.underrun_samples == 0 && self.overrun_samples == 0 && self.retired_drops == 0
    }

    #[must_use]
    pub const fn glitch_samples(&self) -> u64 {
        self.underrun_samples + self.overrun_samples
    }

    #[must_use]
    pub fn latency_ms(&self, sample_rate: u32, channels: u16) -> f64 {
        let samples_per_second = f64::from(sample_rate) * f64::from(channels);
        if samples_per_second <= 0.0 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            let samples = self.latency_samples as f64;
            samples / samples_per_second * 1_000.0
        }
    }
}

#[cfg(test)]
mod tests;
