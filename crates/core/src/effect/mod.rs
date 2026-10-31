#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use crate::audio::{AudioBuffer, AudioFormat};

pub trait Accelerator: Send {
    fn scale(&mut self, buffer: &mut AudioBuffer, gain: f32, deadline_ns: u64) -> bool;
}

pub trait Effect: Send {
    fn id(&self) -> &'static str;

    fn prepare(&mut self, format: AudioFormat);

    fn reset(&mut self);

    fn process(&mut self, buffer: &mut AudioBuffer);

    fn elementwise_gain(&mut self) -> Option<f32> {
        None
    }

    fn latency_frames(&self) -> usize {
        0
    }
}
