use crate::audio::{AudioBuffer, AudioFormat};

pub trait Effect: Send {
    fn id(&self) -> &'static str;

    fn prepare(&mut self, format: AudioFormat);

    fn reset(&mut self);

    fn process(&mut self, buffer: &mut AudioBuffer);

    fn latency_frames(&self) -> u32 {
        0
    }
}
