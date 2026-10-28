//! An allocation-free single-producer / single-consumer ring buffer for `f32`.
//!
//! The virtual-sink engine runs two PipeWire streams. The capture stream fills
//! this buffer, the playback stream drains it. Those two callbacks run on the
//! same realtime thread *in the same process but at different points in the
//! graph cycle*, so the hand-off has to be lock-free and allocation-free.
//!
//! The indices are free-running and wrap naturally through `usize`; the mask
//! keeps them inside the allocation. That avoids the classic "is the buffer
//! full or empty?" ambiguity of a ring that uses `head == tail` for both.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Outcome of a bulk write or read, so callers can count glitches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferReport {
    /// How many samples were actually transferred.
    pub moved: usize,
    /// How many samples were requested but could not be moved.
    pub shortfall: usize,
}

impl TransferReport {
    /// Whether the whole request was satisfied.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.shortfall == 0
    }

    /// Whether nothing could be transferred.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.moved == 0
    }
}

/// A lock-free ring buffer holding interleaved `f32` samples.
///
/// # Thread safety
///
/// Exactly one thread may call [`AudioRing::write`] and exactly one — possibly
/// different — thread may call [`AudioRing::read`]. Any number of threads may
/// call the informational methods.
pub struct AudioRing {
    // SAFETY: slots are only touched by the producer between the tail load and
    // the head store, and by the consumer between the head load and the tail
    // store. Those windows cannot overlap, which is the whole point of an SPSC
    // ring. `Send`/`Sync` are therefore sound to assert manually.
    buffer: Box<[UnsafeCell<f32>]>,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: see the note on the `buffer` field.
unsafe impl Send for AudioRing {}
// SAFETY: see the note on the `buffer` field.
unsafe impl Sync for AudioRing {}

impl AudioRing {
    /// Creates a ring holding at least `capacity` samples.
    ///
    /// The capacity is rounded up to a power of two so that index masking is a
    /// single `and`.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two().max(2);
        let buffer = (0..capacity)
            .map(|_| UnsafeCell::new(0.0_f32))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            buffer,
            mask: capacity - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Creates a ring sized for a duration of stereo audio.
    #[must_use]
    pub fn for_duration(sample_rate: u32, channels: usize, duration: Duration) -> Self {
        let samples =
            (f64::from(sample_rate) * duration.as_secs_f64() * channels as f64).ceil() as usize;
        Self::with_capacity(samples.max(1_024))
    }

    /// The usable capacity in samples.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// How many samples are waiting to be read.
    #[must_use]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail)
    }

    /// Whether there is nothing to read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many samples can still be written.
    #[must_use]
    pub fn spare(&self) -> usize {
        self.capacity() - self.len()
    }

    /// Discards everything currently buffered.
    ///
    /// Safe to call from either side; the other side simply observes an empty
    /// buffer.
    pub fn clear(&self) {
        let head = self.head.load(Ordering::Acquire);
        self.tail.store(head, Ordering::Release);
    }

    /// Producer side: appends as many samples as fit.
    ///
    /// Samples that do not fit are dropped; the caller learns how many via the
    /// returned report.
    pub fn write(&self, samples: &[f32]) -> TransferReport {
        let movable = samples.len().min(self.spare());
        if movable > 0 {
            let head = self.head.load(Ordering::Relaxed);
            for (offset, sample) in samples[..movable].iter().enumerate() {
                let index = head.wrapping_add(offset) & self.mask;
                // SAFETY: this slot is owned by the producer until `head` is
                // published below, and the consumer cannot observe it yet.
                unsafe {
                    *self.buffer[index].get() = *sample;
                }
            }
            self.head
                .store(head.wrapping_add(movable), Ordering::Release);
        }
        TransferReport {
            moved: movable,
            shortfall: samples.len() - movable,
        }
    }

    /// Consumer side: reads as many samples as are available.
    pub fn read(&self, out: &mut [f32]) -> TransferReport {
        let movable = out.len().min(self.len());
        if movable > 0 {
            let tail = self.tail.load(Ordering::Relaxed);
            for (offset, slot) in out[..movable].iter_mut().enumerate() {
                let index = tail.wrapping_add(offset) & self.mask;
                // SAFETY: this slot was published by the producer with a
                // release store and has not been reclaimed, because the
                // consumer only advances `tail` after this loop.
                *slot = unsafe { *self.buffer[index].get() };
            }
            self.tail
                .store(tail.wrapping_add(movable), Ordering::Release);
        }
        TransferReport {
            moved: movable,
            shortfall: out.len() - movable,
        }
    }

    /// Consumer side: discards up to `count` of the oldest samples.
    ///
    /// The engine uses this to trim accumulated latency when the capture and
    /// playback clocks drift apart. Only the consumer can do this safely, which
    /// is why it lives on the reading side.
    pub fn discard_oldest(&self, count: usize) -> usize {
        let discarded = count.min(self.len());
        if discarded > 0 {
            let tail = self.tail.load(Ordering::Relaxed);
            self.tail
                .store(tail.wrapping_add(discarded), Ordering::Release);
        }
        discarded
    }
}

impl std::fmt::Debug for AudioRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioRing")
            .field("capacity", &self.capacity())
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn capacity_is_rounded_up_to_a_power_of_two() {
        assert_eq!(AudioRing::with_capacity(1_000).capacity(), 1_024);
        assert_eq!(AudioRing::with_capacity(1).capacity(), 2);
    }

    #[test]
    fn write_then_read_round_trips() {
        let ring = AudioRing::with_capacity(8);
        let report = ring.write(&[1.0, 2.0, 3.0]);
        assert_eq!(report.moved, 3);
        assert!(report.is_complete());
        assert_eq!(ring.len(), 3);

        let mut out = [0.0; 3];
        assert_eq!(ring.read(&mut out).moved, 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);
        assert!(ring.is_empty());
    }

    #[test]
    fn writes_beyond_capacity_are_truncated_not_wrapped_over() {
        let ring = AudioRing::with_capacity(4);
        let report = ring.write(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(report.moved, 4);
        assert_eq!(report.shortfall, 2);
        let mut out = [0.0; 4];
        ring.read(&mut out);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn reads_past_the_end_leave_the_rest_untouched() {
        let ring = AudioRing::with_capacity(4);
        ring.write(&[7.0, 8.0]);
        let mut out = [9.0; 4];
        let report = ring.read(&mut out);
        assert_eq!(report.moved, 2);
        assert_eq!(report.shortfall, 2);
        assert_eq!(out, [7.0, 8.0, 9.0, 9.0]);
    }

    #[test]
    fn indices_wrap_correctly_across_many_cycles() {
        let ring = AudioRing::with_capacity(4);
        for round in 0..100 {
            #[allow(clippy::cast_precision_loss)]
            let base = round as f32;
            ring.write(&[base, base + 1.0, base + 2.0, base + 3.0]);
            let mut out = [0.0; 4];
            assert_eq!(ring.read(&mut out).moved, 4);
            assert_eq!(out, [base, base + 1.0, base + 2.0, base + 3.0]);
        }
        assert!(ring.is_empty());
    }

    #[test]
    fn discard_oldest_trims_from_the_reading_end() {
        let ring = AudioRing::with_capacity(8);
        ring.write(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(ring.discard_oldest(2), 2);
        assert_eq!(ring.len(), 3);
        let mut out = [0.0; 3];
        ring.read(&mut out);
        assert_eq!(out, [3.0, 4.0, 5.0]);
    }

    #[test]
    fn clear_empties_the_buffer() {
        let ring = AudioRing::with_capacity(8);
        ring.write(&[1.0, 2.0, 3.0]);
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.spare(), ring.capacity());
    }

    #[test]
    fn concurrent_producer_and_consumer_transfer_everything_in_order() {
        const TOTAL_BLOCKS: usize = 20_000;
        const BLOCK: usize = 32;

        let ring = Arc::new(AudioRing::with_capacity(256));
        let producer = {
            let ring = Arc::clone(&ring);
            thread::spawn(move || {
                let mut produced = 0_usize;
                while produced < TOTAL_BLOCKS {
                    let mut block = [0.0_f32; BLOCK];
                    let base = produced * BLOCK;
                    for (offset, slot) in block.iter_mut().enumerate() {
                        #[allow(clippy::cast_precision_loss)]
                        {
                            *slot = (base + offset) as f32;
                        }
                    }
                    if ring.write(&block).is_complete() {
                        produced += 1;
                    } else {
                        thread::yield_now();
                    }
                }
            })
        };

        let consumer = {
            let ring = Arc::clone(&ring);
            thread::spawn(move || {
                let mut consumed = 0_usize;
                let mut block = [0.0_f32; BLOCK];
                while consumed < TOTAL_BLOCKS {
                    let report = ring.read(&mut block);
                    if report.is_complete() {
                        for (offset, sample) in block.iter().enumerate() {
                            #[allow(clippy::cast_precision_loss)]
                            let expected = (consumed * BLOCK + offset) as f32;
                            assert_eq!(*sample, expected, "sample out of order");
                        }
                        consumed += 1;
                    } else {
                        thread::yield_now();
                    }
                }
            })
        };

        producer.join().unwrap();
        consumer.join().unwrap();
    }

    #[test]
    fn for_duration_sizes_the_buffer_for_the_requested_window() {
        let ring = AudioRing::for_duration(48_000, 2, Duration::from_millis(500));
        assert!(ring.capacity() >= 48_000);
    }
}
