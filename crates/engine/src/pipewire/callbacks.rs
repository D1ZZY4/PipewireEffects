use pipewire as pw;
use pw::spa;
use spa::param::ParamType;
use spa::pod::Pod;

use pipewire_effects_core::AudioFormat;

use super::samples::{read_samples, write_samples};
use super::{connection_state, negotiated_format};
use crate::events::{EngineEvent, EventSink, StreamRole};
use crate::processor::{CaptureSource, RtProcessor};

pub(super) struct CaptureCallback {
    pub(super) source: CaptureSource,
    pub(super) format: AudioFormat,
    pub(super) scratch: Vec<f32>,
    pub(super) role: StreamRole,
    pub(super) events: EventSink,
}

impl CaptureCallback {
    pub(super) fn on_param_changed(&mut self, id: u32, param: Option<&Pod>) {
        if id != ParamType::Format.as_raw() {
            return;
        }
        let Some(param) = param else {
            return;
        };
        if let Some(format) = negotiated_format(param) {
            self.events(EngineEvent::FormatNegotiated {
                role: self.role,
                format,
            });
        }
    }

    pub(super) fn on_state_changed(&self, state: pw::stream::StreamState) {
        self.events(EngineEvent::ConnectionChanged {
            role: self.role,
            state: connection_state(state),
        });
    }

    fn events(&self, event: EngineEvent) {
        (self.events)(event);
    }

    pub(super) fn capture(&mut self, bytes: &[u8]) {
        let channels = self.format.channels_usize().max(1);
        let samples = read_samples(bytes, &mut self.scratch, channels);
        self.source.capture(samples);
    }
}

pub(super) struct PlaybackCallback {
    pub(super) processor: RtProcessor,
    pub(super) scratch: Vec<f32>,
    pub(super) channels: usize,
    pub(super) role: StreamRole,
    pub(super) events: EventSink,
}

impl PlaybackCallback {
    pub(super) fn on_param_changed(&mut self, id: u32, param: Option<&Pod>) {
        if id != ParamType::Format.as_raw() {
            return;
        }
        let Some(param) = param else {
            return;
        };
        if let Some(format) = negotiated_format(param) {
            self.events(EngineEvent::FormatNegotiated {
                role: self.role,
                format,
            });
        }
    }

    pub(super) fn on_state_changed(&self, state: pw::stream::StreamState) {
        self.events(EngineEvent::ConnectionChanged {
            role: self.role,
            state: connection_state(state),
        });
    }

    fn events(&self, event: EngineEvent) {
        (self.events)(event);
    }

    pub(super) fn render(&mut self, bytes: &mut [u8], frames: usize) {
        let frames = frames.min(self.scratch.len() / self.channels);
        let needed = frames * self.channels;
        if needed == 0 {
            bytes.fill(0);
            return;
        }
        self.processor.process(&mut self.scratch[..needed], frames);
        write_samples(&self.scratch[..needed], bytes);
    }
}
