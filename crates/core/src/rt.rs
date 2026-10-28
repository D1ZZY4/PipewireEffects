use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferReport {
    pub moved: usize,
    pub shortfall: usize,
}

impl TransferReport {
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.shortfall == 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.moved == 0
    }
}

pub struct AudioRing {
    buffer: Box<[UnsafeCell<f32>]>,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl Send for AudioRing {}
unsafe impl Sync for AudioRing {}

impl AudioRing {
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

    #[must_use]
    pub fn for_duration(sample_rate: u32, channels: usize, duration: Duration) -> Self {
        let samples =
            (f64::from(sample_rate) * duration.as_secs_f64() * channels as f64).ceil() as usize;
        Self::with_capacity(samples.max(1_024))
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn spare(&self) -> usize {
        self.capacity() - self.len()
    }

    pub fn clear(&self) {
        let head = self.head.load(Ordering::Acquire);
        self.tail.store(head, Ordering::Release);
    }

    pub fn write(&self, samples: &[f32]) -> TransferReport {
        let movable = samples.len().min(self.spare());
        if movable > 0 {
            let head = self.head.load(Ordering::Relaxed);
            for (offset, sample) in samples[..movable].iter().enumerate() {
                let index = head.wrapping_add(offset) & self.mask;
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

    pub fn read(&self, out: &mut [f32]) -> TransferReport {
        let movable = out.len().min(self.len());
        if movable > 0 {
            let tail = self.tail.load(Ordering::Relaxed);
            for (offset, slot) in out[..movable].iter_mut().enumerate() {
                let index = tail.wrapping_add(offset) & self.mask;
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
mod tests;
