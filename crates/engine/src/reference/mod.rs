use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

const CHANNELS: usize = 2;

impl PartialEq for ReferenceBus {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl std::fmt::Debug for ReferenceBus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReferenceBus")
            .field("frames", &self.frames)
            .field("write", &self.write.load(Ordering::Relaxed))
            .field("read", &self.read.load(Ordering::Relaxed))
            .finish()
    }
}

pub struct ReferenceBus {
    frames: usize,
    mask: usize,
    write: AtomicUsize,
    read: AtomicUsize,
    samples: Box<[AtomicU32]>,
}

impl ReferenceBus {
    pub fn new(frames: usize) -> Arc<Self> {
        let frames = frames.max(1_024).next_power_of_two();
        Arc::new(Self {
            frames,
            mask: frames - 1,
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
            samples: (0..frames * CHANNELS).map(|_| AtomicU32::new(0)).collect(),
        })
    }

    pub fn capacity_frames(&self) -> usize {
        self.frames
    }

    pub fn push(&self, left: &[f32], right: &[f32]) {
        let frames = left.len().min(right.len());
        let write = self.write.load(Ordering::Relaxed);
        let read = self.read.load(Ordering::Acquire);
        let used = write.saturating_sub(read);
        if used.saturating_add(frames) > self.frames {
            return;
        }
        for index in 0..frames {
            let position = ((write + index) & self.mask) * CHANNELS;
            self.samples[position].store(left[index].to_bits(), Ordering::Relaxed);
            self.samples[position + 1].store(right[index].to_bits(), Ordering::Relaxed);
        }
        if frames > 0 {
            self.write
                .store(write.saturating_add(frames), Ordering::Release);
        }
    }

    pub fn pop(&self, destination: &mut [f32]) -> usize {
        let frames = destination.len() / CHANNELS;
        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Acquire);
        let available = write.saturating_sub(read).min(frames);
        for index in 0..available {
            let position = ((read + index) & self.mask) * CHANNELS;
            destination[index * CHANNELS] =
                f32::from_bits(self.samples[position].load(Ordering::Relaxed));
            destination[index * CHANNELS + 1] =
                f32::from_bits(self.samples[position + 1].load(Ordering::Relaxed));
        }
        for sample in &mut destination[available * CHANNELS..frames * CHANNELS] {
            *sample = 0.0;
        }
        if available > 0 {
            self.read
                .store(read.saturating_add(available), Ordering::Release);
        }
        available
    }

    pub fn clear(&self) {
        let write = self.write.load(Ordering::Acquire);
        self.read.store(write, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
