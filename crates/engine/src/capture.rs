use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use pwe_core::LevelMeter;
use rtrb::Producer;

use crate::config::MAX_QUANTUM_FRAMES;
use crate::shared::Shared;
use crate::streams::bytes_to_samples;

pub struct CaptureState {
    producer: Producer<f32>,
    scratch: Vec<f32>,
    meter: LevelMeter,
    shared: Arc<Shared>,
    channels: usize,
    rate: Arc<AtomicU32>,
    meter_rate: u32,
}

impl CaptureState {
    pub fn new(
        producer: Producer<f32>,
        shared: Arc<Shared>,
        rate: Arc<AtomicU32>,
        channels: usize,
    ) -> Self {
        let mut meter = LevelMeter::new();
        meter.set_rate(rate.load(Ordering::Relaxed).max(8_000));
        Self {
            producer,
            scratch: vec![0.0; MAX_QUANTUM_FRAMES * channels.max(1)],
            meter,
            shared,
            channels: channels.max(1),
            rate,
            meter_rate: 0,
        }
    }

    pub fn submit(&mut self, bytes: &[u8]) -> usize {
        let channels = self.channels;
        let capacity = self.scratch.len();
        let samples = bytes_to_samples(bytes, &mut self.scratch[..capacity]);
        if samples == 0 {
            return 0;
        }
        let rate = self.rate.load(Ordering::Relaxed);
        if rate >= 8_000 && rate != self.meter_rate {
            self.meter_rate = rate;
            self.meter.set_rate(rate);
        }
        let slice = &self.scratch[..samples];
        self.meter.process_interleaved(slice, channels);
        self.shared.meters().publish_input(
            [self.meter.peak_db(0), self.meter.peak_db(1)],
            [self.meter.rms_db(0), self.meter.rms_db(1)],
        );
        let mut pushed = 0;
        for sample in slice {
            if self.producer.push(*sample).is_err() {
                break;
            }
            pushed += 1;
        }
        let frames = samples / channels;
        self.shared.stats().add_captured(frames as u64);
        if pushed < samples {
            self.shared.stats().add_overrun((samples - pushed) as u64);
        }
        frames
    }
}
