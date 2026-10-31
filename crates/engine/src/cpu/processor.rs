use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crossbeam_queue::ArrayQueue;
use pwe_core::i18n::t;
use pwe_core::{AudioBuffer, AudioFormat, ChainCommand, Effect, EffectChain, LevelMeter};
use pwe_dsp::SpectrumAnalyzer;
use rtrb::Consumer;

use crate::config::MAX_QUANTUM_FRAMES;
use crate::shared::Shared;

pub struct Processor {
    chain: EffectChain,
    commands: Arc<ArrayQueue<ChainCommand>>,
    retired: Arc<ArrayQueue<Box<dyn Effect>>>,
    held: Vec<Box<dyn Effect>>,
    shared: Arc<Shared>,
    rate: Arc<AtomicU32>,
    format: AudioFormat,
    buffer: AudioBuffer,
    scratch: Vec<f32>,
    target_samples: usize,
    channels: usize,
    meter: LevelMeter,
    analyser: SpectrumAnalyzer,
}

impl Processor {
    pub fn new(
        commands: Arc<ArrayQueue<ChainCommand>>,
        retired: Arc<ArrayQueue<Box<dyn Effect>>>,
        shared: Arc<Shared>,
        rate: Arc<AtomicU32>,
        channels: usize,
        backend: crate::gpu::AudioBackend,
    ) -> Self {
        let channels = channels.max(1);
        let rate_value = rate.load(Ordering::Relaxed).max(48_000);
        let format = AudioFormat::new(rate_value, channels);
        let mut meter = LevelMeter::new();
        meter.set_rate(rate_value);
        let mut chain = EffectChain::new(crate::config::MAX_EFFECTS, format);
        let accelerator = backend.accelerator(MAX_QUANTUM_FRAMES * channels, Arc::clone(&shared));
        if backend != crate::gpu::AudioBackend::Cpu && accelerator.is_none() {
            shared.add_warning(
                t("No Vulkan compute device is available; the chain runs on the CPU").to_owned(),
            );
        }
        chain.set_accelerator(accelerator);
        let mut analyser = SpectrumAnalyzer::new();
        analyser.prepare(rate_value);
        Self {
            chain,
            commands,
            retired,
            held: Vec::with_capacity(crate::engine::RETIRE_QUEUE_CAPACITY),
            shared,
            rate,
            format,
            buffer: AudioBuffer::new(channels, MAX_QUANTUM_FRAMES),
            scratch: vec![0.0; MAX_QUANTUM_FRAMES * channels],
            target_samples: channels * (rate_value as usize / 32),
            channels,
            meter,
            analyser,
        }
    }

    pub fn prepare(&mut self, format: AudioFormat) {
        self.chain.prepare(format);
        self.analyser.prepare(format.rate());
        self.format = format;
    }

    pub fn process(
        &mut self,
        consumer: &mut Consumer<f32>,
        frames: usize,
        destination: &mut [f32],
    ) {
        self.drain_commands();
        self.follow_rate();
        self.shared
            .stats()
            .set_chain_latency_frames(self.chain.latency_frames() as u64);
        let channels = self.channels;
        let wanted = (frames * channels).min(self.scratch.len());
        self.trim(consumer);

        let available = consumer.slots().min(wanted);
        let mut filled = 0;
        while filled < available {
            match consumer.pop() {
                Ok(sample) => {
                    self.scratch[filled] = sample;
                    filled += 1;
                }
                Err(_) => break,
            }
        }
        if filled < wanted {
            for sample in &mut self.scratch[filled..wanted] {
                *sample = 0.0;
            }
            self.shared.stats().add_underrun((wanted - filled) as u64);
        }

        let processed_frames = wanted / channels;
        if self.buffer.configure(channels, processed_frames).is_err() {
            self.shared.add_warning(
                t("the graph quantum is larger than the processing buffer").to_owned(),
            );
            return;
        }
        self.buffer.deinterleave_from(&self.scratch[..wanted]);
        self.chain.process(&mut self.buffer);
        self.buffer.sanitize();
        self.publish_output();
        self.publish_spectrum();
        self.shared.stats().add_processed(processed_frames as u64);
        self.shared
            .stats()
            .set_buffer_frames(processed_frames as u64);
        self.shared
            .stats()
            .set_ring_occupancy(consumer.slots() as u64);
        self.buffer.interleave_into(destination);
    }

    pub fn drain_commands(&mut self) {
        self.flush_held();
        let retired = &self.retired;
        let held = &mut self.held;
        let mut budget = 128;
        while budget > 0 {
            let Some(command) = self.commands.pop() else {
                break;
            };
            budget -= 1;
            self.chain.apply(command, |effect| {
                if let Err(effect) = retired.push(effect) {
                    held.push(effect);
                }
            });
        }
    }

    fn flush_held(&mut self) {
        while let Some(effect) = self.held.pop() {
            if let Err(effect) = self.retired.push(effect) {
                self.held.push(effect);
                break;
            }
        }
    }

    fn trim(&mut self, consumer: &mut Consumer<f32>) {
        let occupancy = consumer.slots();
        let limit = self.target_samples + self.target_samples / 2;
        if occupancy > limit {
            let excess = occupancy - self.target_samples;
            for _ in 0..excess {
                if consumer.pop().is_err() {
                    break;
                }
            }
            self.shared.stats().record_trim();
        }
    }

    fn follow_rate(&mut self) {
        let rate = self.rate.load(Ordering::Relaxed);
        if rate < 8_000 || rate == self.format.rate() {
            return;
        }
        self.format = AudioFormat::new(rate, self.channels);
        self.chain.prepare(self.format);
        self.analyser.prepare(rate);
        self.meter.set_rate(rate);
        self.target_samples = self.channels * (rate as usize / 32);
    }

    fn publish_output(&mut self) {
        self.meter.process(&self.buffer);
        let peaks = [self.meter.peak_db(0), self.meter.peak_db(1)];
        let rms = [self.meter.rms_db(0), self.meter.rms_db(1)];
        self.shared.meters().publish_output(peaks, rms);
    }

    fn publish_spectrum(&mut self) {
        if let Some(values) = self.analyser.push(&self.buffer) {
            self.shared.spectrum().publish(&values);
        }
    }
}

#[cfg(test)]
mod tests;
