//! Counters describing how well the audio path is behaving.
//!
//! Every field is an atomic, written by the audio thread and read by the
//! interface, so a reading is a snapshot rather than a consistent view. That is
//! fine for the question these counters answer — "is audio flowing, and is it
//! glitching?" — and it keeps the audio thread free of locks.
//!
//! The counters are the honest answer to "did it work": a loopback can appear
//! to run while dropping a few milliseconds of audio every second, and the only
//! way to notice is to count.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Shared counters for one engine instance.
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
    /// Creates a counter bank in a reference-counted handle.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Records `frames` captured from the virtual sink.
    pub fn add_capture_frames(&self, frames: u64) {
        self.capture_frames.fetch_add(frames, Ordering::Relaxed);
    }

    /// Records `frames` processed and played.
    pub fn add_processed_frames(&self, frames: u64) {
        self.processed_frames.fetch_add(frames, Ordering::Relaxed);
    }

    /// Records samples that the playback side needed but did not have.
    pub fn add_underrun_samples(&self, samples: u64) {
        self.underrun_samples.fetch_add(samples, Ordering::Relaxed);
    }

    /// Records samples the capture side had to drop because the ring was full.
    pub fn add_overrun_samples(&self, samples: u64) {
        self.overrun_samples.fetch_add(samples, Ordering::Relaxed);
    }

    /// Records an effect that could not be handed off for disposal.
    ///
    /// This means the interface thread is not draining the recycle queue, which
    /// only happens if it is blocked or gone. Dropping the instance on the audio
    /// thread is the fallback.
    pub fn add_retired_drop(&self) {
        self.retired_drops.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a drift correction.
    pub fn add_trim(&self, samples: u64) {
        self.trim_events.fetch_add(1, Ordering::Relaxed);
        self.trimmed_samples.fetch_add(samples, Ordering::Relaxed);
    }

    /// Publishes the current ring occupancy.
    pub fn set_latency_samples(&self, samples: u64) {
        self.latency_samples.store(samples, Ordering::Relaxed);
    }

    /// Reads the counters.
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

    /// Clears every counter. The totals are not reset, only the problems.
    pub fn reset_problems(&self) {
        self.underrun_samples.store(0, Ordering::Relaxed);
        self.overrun_samples.store(0, Ordering::Relaxed);
        self.retired_drops.store(0, Ordering::Relaxed);
        self.trim_events.store(0, Ordering::Relaxed);
        self.trimmed_samples.store(0, Ordering::Relaxed);
    }
}

/// A point-in-time reading of [`StreamStats`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatsSnapshot {
    /// Frames received from applications.
    pub capture_frames: u64,
    /// Frames processed and played out.
    pub processed_frames: u64,
    /// Samples the playback side had to invent because the buffer was empty.
    pub underrun_samples: u64,
    /// Samples the capture side had to discard because the buffer was full.
    pub overrun_samples: u64,
    /// Effects that could not be handed back for disposal.
    pub retired_drops: u64,
    /// How many times accumulated latency was trimmed.
    pub trim_events: u64,
    /// Total samples trimmed.
    pub trimmed_samples: u64,
    /// Current ring occupancy, in samples.
    pub latency_samples: u64,
}

impl StatsSnapshot {
    /// Whether audio is flowing.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.capture_frames > 0 && self.processed_frames > 0
    }

    /// Whether the engine has had to paper over a problem.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.underrun_samples == 0 && self.overrun_samples == 0 && self.retired_drops == 0
    }

    /// Total samples that were lost or invented.
    #[must_use]
    pub const fn glitch_samples(&self) -> u64 {
        self.underrun_samples + self.overrun_samples
    }

    /// The current buffer occupancy, in milliseconds.
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
mod tests {
    use super::*;

    #[test]
    fn a_fresh_bank_is_idle_and_healthy() {
        let stats = StreamStats::new();
        let snapshot = stats.snapshot();
        assert!(!snapshot.is_running());
        assert!(snapshot.is_healthy());
        assert_eq!(snapshot.glitch_samples(), 0);
    }

    #[test]
    fn counters_accumulate_independently() {
        let stats = StreamStats::new();
        stats.add_capture_frames(480);
        stats.add_capture_frames(480);
        stats.add_processed_frames(960);
        stats.add_underrun_samples(64);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.capture_frames, 960);
        assert_eq!(snapshot.processed_frames, 960);
        assert_eq!(snapshot.underrun_samples, 64);
        assert!(snapshot.is_running());
        assert!(!snapshot.is_healthy());
        assert_eq!(snapshot.glitch_samples(), 64);
    }

    #[test]
    fn trims_are_counted_as_events_and_samples() {
        let stats = StreamStats::new();
        stats.add_trim(1_024);
        stats.add_trim(512);
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.trim_events, 2);
        assert_eq!(snapshot.trimmed_samples, 1_536);
        // A trim is a deliberate latency correction, not a glitch.
        assert!(snapshot.is_healthy());
    }

    #[test]
    fn latency_is_reported_in_milliseconds() {
        let stats = StreamStats::new();
        stats.set_latency_samples(1_920);
        let snapshot = stats.snapshot();
        assert!((snapshot.latency_ms(48_000, 2) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn latency_of_a_degenerate_format_is_zero_rather_than_infinite() {
        let snapshot = StatsSnapshot {
            latency_samples: 1_920,
            ..StatsSnapshot::default()
        };
        assert_eq!(snapshot.latency_ms(0, 0), 0.0);
    }

    #[test]
    fn resetting_clears_problems_but_keeps_totals() {
        let stats = StreamStats::new();
        stats.add_capture_frames(480);
        stats.add_processed_frames(480);
        stats.add_overrun_samples(32);
        stats.reset_problems();

        let snapshot = stats.snapshot();
        assert!(snapshot.is_healthy());
        assert_eq!(snapshot.capture_frames, 480);
        assert_eq!(snapshot.overrun_samples, 0);
    }
}
