use super::*;
use pipewire_effects_core::{
    EffectCategory, EffectDef, EffectKind, ParamSet, ParamSpec, ParamUnit,
};

struct Recorder {
    params: ParamSet,
}

impl Recorder {
    fn new() -> Self {
        Self {
            params: ParamSet::new(vec![ParamSpec::float(
                "gain",
                "Gain",
                0.0,
                4.0,
                1.0,
                0.1,
                ParamUnit::Ratio,
            )]),
        }
    }
}

impl AudioEffect for Recorder {
    fn kind(&self) -> EffectKind {
        EffectKind::new("test-recorder")
    }
    fn parameters(&self) -> &ParamSet {
        &self.params
    }
    fn prepare(&mut self, _format: AudioFormat) {}
    fn reset(&mut self) {}
    fn process(&mut self, block: &mut AudioBlock<'_>) {
        block.apply_gain(self.params.value_f32(0));
    }
}

fn factory() -> Box<dyn AudioEffect> {
    Box::new(Recorder::new())
}

static REGISTRY: [EffectDef; 1] = [EffectDef::new(
    EffectKind::new("test-recorder"),
    "Recorder",
    "Test effect.",
    EffectCategory::Utility,
    factory,
)];

const CHANNELS: usize = 2;
const TARGET_FILL: usize = 1_024;

struct Harness {
    processor: RtProcessor,
    commands: Arc<ArrayQueue<ChainCommand>>,
    ring: Arc<AudioRing>,
    stats: Arc<StreamStats>,
    meters: Arc<SharedMeters>,
    _recycle: Arc<ArrayQueue<Box<dyn AudioEffect>>>,
}

impl Harness {
    fn new() -> Self {
        let format = AudioFormat::new(48_000, CHANNELS as u16);
        let commands = Arc::new(ArrayQueue::new(256));
        let recycle = recycle_queue();
        let ring = Arc::new(AudioRing::with_capacity(16_384));
        let meters = SharedMeters::new(CHANNELS);
        let stats = StreamStats::new();

        let processor = RtProcessor::new(
            format,
            Arc::clone(&commands),
            Arc::clone(&recycle),
            Arc::clone(&ring),
            Arc::clone(&meters),
            Arc::clone(&stats),
            TARGET_FILL,
            TARGET_FILL * 2,
        );

        Self {
            processor,
            commands,
            ring,
            stats,
            meters,
            _recycle: recycle,
        }
    }

    fn prime(&self, frames: usize, value: f32) {
        let samples = vec![value; frames * CHANNELS];
        self.ring.write(&samples);
    }

    fn run(&mut self, frames: usize) -> (Vec<f32>, ProcessorReport) {
        let mut buffer = vec![0.0_f32; frames * CHANNELS];
        let report = self.processor.process(&mut buffer, frames);
        (buffer, report)
    }
}

#[test]
fn an_empty_ring_produces_silence_and_is_reported_as_an_underrun() {
    let mut harness = Harness::new();
    let (buffer, report) = harness.run(480);

    assert_eq!(report.frames, 480);
    assert_eq!(report.sampled, 0);
    assert_eq!(report.underrun_samples, 480 * CHANNELS);
    assert!(!report.is_clean());
    assert!(buffer.iter().all(|s| *s == 0.0));
    assert_eq!(harness.stats.snapshot().underrun_samples, 960);
}

#[test]
fn audio_round_trips_through_the_ring_and_the_chain() {
    let mut harness = Harness::new();
    harness.prime(480, 0.25);

    let (buffer, report) = harness.run(480);
    assert_eq!(report.sampled, 960);
    assert!(report.is_clean());
    assert!(buffer.iter().all(|s| (*s - 0.25).abs() < 1e-9));

    let snapshot = harness.stats.snapshot();
    assert_eq!(snapshot.processed_frames, 480);
    assert!(snapshot.is_healthy());
}

#[test]
fn output_meters_reflect_the_processed_signal() {
    let mut harness = Harness::new();
    harness.prime(480, 0.5);
    harness.run(480);

    let snapshot = harness.meters.snapshot();
    assert_eq!(snapshot.channels(), CHANNELS);
    assert!((snapshot.peak_of(0) - 0.5).abs() < 1e-6);
    assert!((snapshot.rms_of(0) - 0.5).abs() < 1e-6);
}

#[test]
fn queued_commands_change_the_chain() {
    let mut harness = Harness::new();
    assert_eq!(harness.processor.chain().len(), 0);

    harness
        .commands
        .push(ChainCommand::Insert {
            index: 0,
            effect: Box::new(Recorder::new()),
            enabled: true,
        })
        .ok()
        .unwrap();

    harness.prime(480, 0.25);
    let (_, report) = harness.run(480);

    assert_eq!(report.commands_applied, 1);
    assert_eq!(harness.processor.chain().len(), 1);
}

#[test]
fn effect_parameters_take_effect_immediately() {
    let mut harness = Harness::new();
    let effect = Box::new(Recorder::new());
    let parameters = effect.parameters().clone();
    harness
        .commands
        .push(ChainCommand::Insert {
            index: 0,
            effect,
            enabled: true,
        })
        .ok()
        .unwrap();

    parameters.set_f32(0, 2.0);
    harness.prime(480, 0.25);
    let (buffer, _) = harness.run(480);

    assert!(buffer.iter().all(|s| (*s - 0.5).abs() < 1e-9));
}

#[test]
fn removed_effects_are_recycled_rather_than_dropped() {
    let mut harness = Harness::new();
    harness
        .commands
        .push(ChainCommand::Insert {
            index: 0,
            effect: Box::new(Recorder::new()),
            enabled: true,
        })
        .ok()
        .unwrap();
    harness.prime(480, 0.0);
    harness.run(480);

    harness
        .commands
        .push(ChainCommand::Remove { index: 0 })
        .ok()
        .unwrap();
    harness.run(480);

    assert!(harness.processor.chain().is_empty());
    assert_eq!(
        harness.processor.recycle_queue().len(),
        1,
        "the retired effect should be waiting for the interface thread"
    );
    assert_eq!(harness.stats.snapshot().retired_drops, 0);
}

#[test]
fn accumulated_latency_is_trimmed_back_to_the_target() {
    let mut harness = Harness::new();
    harness.prime(TARGET_FILL * 6 / CHANNELS, 0.1);
    assert!(harness.ring.len() > TARGET_FILL * 2);

    let (_, report) = harness.run(480);
    assert!(report.trimmed_samples > 0);
    assert_eq!(harness.stats.snapshot().trim_events, 1);
    assert!(harness.ring.len() <= TARGET_FILL);
}

#[test]
fn trimming_is_counted_but_is_not_a_glitch() {
    let mut harness = Harness::new();
    harness.prime(TARGET_FILL * 6 / CHANNELS, 0.1);
    harness.run(480);
    let snapshot = harness.stats.snapshot();
    assert!(snapshot.trimmed_samples > 0);
    assert!(snapshot.is_healthy());
    assert_eq!(snapshot.glitch_samples(), 0);
}

#[test]
fn a_block_larger_than_the_buffer_is_clamped_rather_than_panicking() {
    let mut harness = Harness::new();
    let mut buffer = vec![0.0_f32; 64];
    let report = harness.processor.process(&mut buffer, 100_000);
    assert_eq!(report.frames, 32);
    assert_eq!(report.underrun_samples, 64);
}

#[test]
fn a_zero_frame_block_is_harmless() {
    let mut harness = Harness::new();
    let mut buffer = vec![0.0_f32; 8];
    let report = harness.processor.process(&mut buffer, 0);
    assert_eq!(report.frames, 0);
    assert_eq!(report.sampled, 0);
    assert!(report.is_clean());
}

#[test]
fn hostile_samples_do_not_escape_the_processor() {
    let mut harness = Harness::new();
    let mut samples = vec![0.0_f32; 480 * CHANNELS];
    samples[0] = f32::NAN;
    samples[1] = f32::INFINITY;
    harness.ring.write(&samples);

    let (buffer, _) = harness.run(480);
    assert!(
        buffer.iter().all(|s| s.is_finite()),
        "a non-finite sample reached the output"
    );
}

#[test]
fn the_capture_source_fills_the_ring_and_meters_the_input() {
    let ring = Arc::new(AudioRing::with_capacity(4_096));
    let meters = SharedMeters::new(CHANNELS);
    let stats = StreamStats::new();
    let mut source = CaptureSource::new(
        Arc::clone(&ring),
        Arc::clone(&meters),
        Arc::clone(&stats),
        AudioFormat::new(48_000, CHANNELS as u16),
    );

    let written = source.capture(&[0.75; 480 * CHANNELS]);
    assert_eq!(written, 480 * CHANNELS);
    assert_eq!(ring.len(), 960);
    assert_eq!(stats.snapshot().capture_frames, 480);
    assert!((meters.snapshot().peak_of(0) - 0.75).abs() < 1e-6);
}

#[test]
fn the_capture_source_drops_rather_than_overwrites_when_full() {
    let ring = Arc::new(AudioRing::with_capacity(64));
    let meters = SharedMeters::new(CHANNELS);
    let stats = StreamStats::new();
    let mut source = CaptureSource::new(
        Arc::clone(&ring),
        Arc::clone(&meters),
        Arc::clone(&stats),
        AudioFormat::new(48_000, CHANNELS as u16),
    );

    source.capture(&[0.5; 64]);
    source.capture(&[0.5; 64]);

    assert_eq!(ring.len(), 64);
    assert_eq!(stats.snapshot().overrun_samples, 64);
}

#[test]
fn a_registry_effect_can_be_inserted_by_command() {
    let mut harness = Harness::new();
    let effect = REGISTRY[0].instantiate();
    harness
        .commands
        .push(ChainCommand::Insert {
            index: usize::MAX,
            effect,
            enabled: true,
        })
        .ok()
        .unwrap();
    harness.prime(480, 0.1);
    harness.run(480);
    assert_eq!(harness.processor.chain().len(), 1);
}
