use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crossbeam_queue::ArrayQueue;
use pwe_core::i18n::t;
use pwe_core::{AudioBuffer, AudioFormat, ChainCommand, Effect, EffectChain, LevelMeter};
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
}

impl Processor {
    pub fn new(
        commands: Arc<ArrayQueue<ChainCommand>>,
        retired: Arc<ArrayQueue<Box<dyn Effect>>>,
        shared: Arc<Shared>,
        rate: Arc<AtomicU32>,
        channels: usize,
    ) -> Self {
        let channels = channels.max(1);
        let rate_value = rate.load(Ordering::Relaxed).max(48_000);
        let format = AudioFormat::new(rate_value, channels);
        let mut meter = LevelMeter::new();
        meter.set_rate(rate_value);
        Self {
            chain: EffectChain::new(crate::config::MAX_EFFECTS, format),
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
        }
    }

    pub fn prepare(&mut self, format: AudioFormat) {
        self.chain.prepare(format);
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
        self.meter.set_rate(rate);
        self.target_samples = self.channels * (rate as usize / 32);
    }

    fn publish_output(&mut self) {
        self.meter.process(&self.buffer);
        let peaks = [self.meter.peak_db(0), self.meter.peak_db(1)];
        let rms = [self.meter.rms_db(0), self.meter.rms_db(1)];
        self.shared.meters().publish_output(peaks, rms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CaptureState;
    use crate::shared::Shared;

    #[test]
    fn a_silent_ring_produces_silence_and_counts_underruns() {
        let shared = Arc::new(Shared::new());
        let commands = Arc::new(ArrayQueue::new(8));
        let retired = Arc::new(ArrayQueue::new(8));
        let rate = Arc::new(AtomicU32::new(48_000));
        let (_producer, mut consumer) = rtrb::RingBuffer::new(4096);
        let mut processor = Processor::new(commands, retired, Arc::clone(&shared), rate, 2);

        let mut destination = vec![0.0f32; 2048];
        processor.process(&mut consumer, 1024, &mut destination);
        assert_eq!(shared.stats().underrun_samples(), 2048);
        assert!(destination.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn audio_written_to_the_ring_reaches_the_output() {
        let shared = Arc::new(Shared::new());
        let commands = Arc::new(ArrayQueue::new(8));
        let retired = Arc::new(ArrayQueue::new(8));
        let rate = Arc::new(AtomicU32::new(48_000));
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(8192);
        let mut processor = Processor::new(commands, retired, Arc::clone(&shared), rate, 2);

        for value in [0.5f32, -0.5].iter().cycle().take(2048) {
            producer.push(*value).unwrap();
        }
        let mut destination = vec![0.0f32; 2048];
        processor.process(&mut consumer, 1024, &mut destination);
        assert!((destination[0] - 0.5).abs() < 1.0e-6);
        assert!((destination[1] + 0.5).abs() < 1.0e-6);
        assert_eq!(shared.stats().processed_frames(), 1024);
    }

    #[test]
    fn an_excessively_full_ring_is_trimmed_back_to_the_target() {
        let shared = Arc::new(Shared::new());
        let commands = Arc::new(ArrayQueue::new(8));
        let retired = Arc::new(ArrayQueue::new(8));
        let rate = Arc::new(AtomicU32::new(48_000));
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(16_384);
        let mut processor = Processor::new(commands, retired, Arc::clone(&shared), rate, 2);
        processor.follow_rate();
        processor.target_samples = 480;

        for _ in 0..8000 {
            producer.push(0.25).unwrap();
        }
        let mut destination = vec![0.0f32; 256];
        processor.process(&mut consumer, 128, &mut destination);
        assert!(shared.stats().trim_events() >= 1);
        assert!(consumer.slots() <= 480 + 128 + 1);
    }

    #[test]
    fn commands_add_and_remove_effects() {
        use pwe_core::{ChainCommand, ParamSet};

        let shared = Arc::new(Shared::new());
        let commands = Arc::new(ArrayQueue::new(8));
        let retired: Arc<ArrayQueue<Box<dyn Effect>>> = Arc::new(ArrayQueue::new(8));
        let rate = Arc::new(AtomicU32::new(48_000));
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(8192);
        let mut processor = Processor::new(
            commands.clone(),
            retired.clone(),
            Arc::clone(&shared),
            rate,
            2,
        );

        let params = ParamSet::new(pwe_dsp_specs());
        params.set_f32("gain", -6.0).unwrap();
        let effect = Box::new(pwe_dsp::effects::Gain::new(params));
        assert!(commands.push(ChainCommand::Add(effect)).is_ok());

        for _ in 0..2048 {
            producer.push(0.5).unwrap();
        }
        let mut destination = vec![0.0f32; 256];
        processor.process(&mut consumer, 128, &mut destination);
        assert!(destination[0] < 0.5, "{}", destination[0]);

        assert!(commands.push(ChainCommand::Remove(0)).is_ok());
        processor.drain_commands();
        assert_eq!(retired.len(), 1);
    }

    #[test]
    fn an_empty_chain_reports_the_same_level_at_both_meters() {
        use crate::streams::samples_to_bytes;

        let shared = Arc::new(Shared::new());
        let commands = Arc::new(ArrayQueue::new(8));
        let retired: Arc<ArrayQueue<Box<dyn Effect>>> = Arc::new(ArrayQueue::new(8));
        let rate = Arc::new(AtomicU32::new(48_000));
        let (producer, mut consumer) = rtrb::RingBuffer::new(8192);
        let mut capture = CaptureState::new(producer, Arc::clone(&shared), Arc::clone(&rate), 2);
        let mut processor =
            Processor::new(commands, retired, Arc::clone(&shared), Arc::clone(&rate), 2);

        let frames = 1024;
        let samples: Vec<f32> = (0..frames)
            .flat_map(|frame| {
                let phase = std::f32::consts::TAU * 997.0 * frame as f32 / 48_000.0;
                [0.5 * phase.sin(), 0.5 * phase.sin()]
            })
            .collect();
        let mut bytes = vec![0u8; samples.len() * 4];
        assert_eq!(samples_to_bytes(&samples, &mut bytes), samples.len());
        assert_eq!(capture.submit(&bytes), frames);

        let mut destination = vec![0.0f32; frames * 2];
        processor.process(&mut consumer, frames, &mut destination);

        let meters = shared.meters().snapshot();
        assert!(
            (meters.input_peak_db[0] - meters.output_peak_db[0]).abs() < 1.0,
            "input {} dB and output {} dB disagree without any effect",
            meters.input_peak_db[0],
            meters.output_peak_db[0]
        );
        assert!(
            (meters.input_rms_db[0] - meters.output_rms_db[0]).abs() < 1.0,
            "input {} dB and output {} dB disagree without any effect",
            meters.input_rms_db[0],
            meters.output_rms_db[0]
        );
    }

    fn pwe_dsp_specs() -> Vec<pwe_core::ParamSpec> {
        pwe_dsp::find("gain").map_or_else(Vec::new, |descriptor| descriptor.specs())
    }
}
