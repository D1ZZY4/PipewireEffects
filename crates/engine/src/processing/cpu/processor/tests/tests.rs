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
    let mut processor = Processor::new(
        commands,
        retired,
        Arc::clone(&shared),
        rate,
        2,
        crate::gpu::AudioBackend::Cpu,
    );

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
    let mut processor = Processor::new(
        commands,
        retired,
        Arc::clone(&shared),
        rate,
        2,
        crate::gpu::AudioBackend::Cpu,
    );

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
    let mut processor = Processor::new(
        commands,
        retired,
        Arc::clone(&shared),
        rate,
        2,
        crate::gpu::AudioBackend::Cpu,
    );
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
        crate::gpu::AudioBackend::Cpu,
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
    let mut processor = Processor::new(
        commands,
        retired,
        Arc::clone(&shared),
        Arc::clone(&rate),
        2,
        crate::gpu::AudioBackend::Cpu,
    );

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

#[test]
fn the_analyser_publishes_a_frame_for_a_tone() {
    let shared = Arc::new(Shared::new());
    let commands = Arc::new(ArrayQueue::new(8));
    let retired = Arc::new(ArrayQueue::new(8));
    let rate = Arc::new(AtomicU32::new(48_000));
    let (mut producer, mut consumer) = rtrb::RingBuffer::new(65_536);
    let mut processor = Processor::new(
        commands,
        retired,
        Arc::clone(&shared),
        rate,
        2,
        crate::gpu::AudioBackend::Cpu,
    );

    let mut destination = vec![0.0f32; 2048];
    let mut frame = 0usize;
    for _ in 0..12 {
        for _ in 0..1024 {
            let phase = std::f32::consts::TAU * 1_000.0 * frame as f32 / 48_000.0;
            producer.push(phase.sin()).unwrap();
            producer.push(phase.sin()).unwrap();
            frame += 1;
        }
        processor.process(&mut consumer, 1024, &mut destination);
    }

    let snapshot = shared.spectrum().snapshot();
    let (index, peak) = snapshot
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map_or((0, f32::NEG_INFINITY), |(index, value)| (index, *value));
    assert!(
        peak.abs() < 3.0,
        "the tone should read 0 dB, peak was {peak}"
    );
    let centre = pwe_dsp::band_centre(index);
    assert!(
        (centre - 1_000.0).abs() < 250.0,
        "the tone should be in its band, it was in the band at {centre} Hz"
    );
}

fn pwe_dsp_specs() -> Vec<pwe_core::ParamSpec> {
    pwe_dsp::find("gain").map_or_else(Vec::new, |descriptor| descriptor.specs())
}

#[test]
fn the_loudness_meter_reports_the_processed_output() {
    let shared = Arc::new(Shared::new());
    let commands = Arc::new(ArrayQueue::new(8));
    let retired = Arc::new(ArrayQueue::new(8));
    let rate = Arc::new(AtomicU32::new(48_000));
    let (mut producer, mut consumer) = rtrb::RingBuffer::new(8192);
    let mut processor = Processor::new(
        commands,
        retired,
        Arc::clone(&shared),
        rate,
        2,
        crate::gpu::AudioBackend::Cpu,
    );

    let mut destination = vec![0.0f32; 2048];
    let mut phase = 0.0f32;
    for _ in 0..160 {
        for _ in 0..2048 {
            producer.push(0.5 * phase.sin()).unwrap();
            phase += std::f32::consts::TAU * 997.0 / 48_000.0;
        }
        processor.process(&mut consumer, 1024, &mut destination);
    }

    let snapshot = shared.meters().snapshot();
    let short_term = snapshot.loudness_short_term_lufs;
    assert!(
        short_term.is_finite(),
        "the short-term loudness was never measured: {short_term}"
    );
    assert!(
        (-30.0..0.0).contains(&short_term),
        "a half-scale tone should read near -9 LUFS, it read {short_term}"
    );
}
