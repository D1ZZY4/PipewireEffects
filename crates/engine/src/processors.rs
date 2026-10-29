use pipewire::stream::Stream;
use rtrb::Consumer;

use crate::capture::CaptureState;
use crate::processor::Processor;
use crate::streams::samples_to_bytes;

pub struct StreamCapture {
    pub state: CaptureState,
}

impl StreamCapture {
    pub fn process(&mut self, stream: &Stream) {
        let Some(mut buffer) = stream.dequeue_buffer() else {
            return;
        };
        let Some(data) = buffer.datas_mut().first_mut() else {
            return;
        };
        let declared = data.chunk().size() as usize;
        let Some(bytes) = data.data() else {
            return;
        };
        let usable = declared.min(bytes.len());
        if usable < 4 {
            return;
        }
        self.state.submit(&bytes[..usable]);
    }
}

pub struct PlaybackState {
    processor: Processor,
    consumer: Consumer<f32>,
    scratch: Vec<f32>,
    channels: usize,
}

impl PlaybackState {
    pub fn new(
        processor: Processor,
        consumer: Consumer<f32>,
        channels: usize,
        capacity: usize,
    ) -> Self {
        Self {
            processor,
            consumer,
            scratch: vec![0.0; capacity.max(1)],
            channels: channels.max(1),
        }
    }

    pub fn process(&mut self, stream: &Stream) {
        let Self {
            processor,
            consumer,
            scratch,
            channels,
        } = self;
        let Some(mut buffer) = stream.dequeue_buffer() else {
            return;
        };
        let requested = buffer.requested() as usize;
        let Some(data) = buffer.datas_mut().first_mut() else {
            return;
        };
        let Some(bytes) = data.data() else {
            return;
        };
        let available_frames = (bytes.len() / 4 / *channels).min(scratch.len() / *channels);
        if available_frames == 0 {
            return;
        }
        let frames = if requested == 0 {
            available_frames
        } else {
            requested.min(available_frames)
        };
        let wanted = frames * *channels;
        let destination = &mut scratch[..wanted];
        processor.process(consumer, frames, destination);
        let written = samples_to_bytes(destination, bytes);
        let chunk = data.chunk_mut();
        *chunk.offset_mut() = 0;
        *chunk.stride_mut() = (*channels * 4) as i32;
        *chunk.size_mut() = (written * 4) as u32;
    }
}
