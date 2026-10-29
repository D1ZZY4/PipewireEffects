use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use pwe_core::PipelineKind;

use crate::command::EffectSlot;
use crate::device::DeviceInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Stopped,
    Starting,
    Running,
    Failed,
}

impl EngineState {
    pub fn label(self) -> &'static str {
        pwe_core::i18n::t(match self {
            Self::Stopped => "Stopped",
            Self::Starting => "Starting",
            Self::Running => "Running",
            Self::Failed => "Failed",
        })
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Starting,
            2 => Self::Running,
            3 => Self::Failed,
            _ => Self::Stopped,
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::Stopped => 0,
            Self::Starting => 1,
            Self::Running => 2,
            Self::Failed => 3,
        }
    }
}

macro_rules! counter {
    ($name:ident) => {
        pub fn $name(&self) -> u64 {
            self.$name.load(Ordering::Relaxed)
        }
    };
}

#[derive(Debug, Default)]
pub struct Stats {
    captured_frames: AtomicU64,
    processed_frames: AtomicU64,
    underrun_samples: AtomicU64,
    overrun_samples: AtomicU64,
    trim_events: AtomicU64,
    buffer_frames: AtomicU64,
    ring_occupancy: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatsSnapshot {
    pub captured_frames: u64,
    pub processed_frames: u64,
    pub underrun_samples: u64,
    pub overrun_samples: u64,
    pub trim_events: u64,
    pub buffer_frames: u64,
    pub ring_occupancy: u64,
}

impl Stats {
    counter!(captured_frames);
    counter!(processed_frames);
    counter!(underrun_samples);
    counter!(overrun_samples);
    counter!(trim_events);
    counter!(buffer_frames);
    counter!(ring_occupancy);

    pub fn add_captured(&self, frames: u64) {
        self.captured_frames.fetch_add(frames, Ordering::Relaxed);
    }

    pub fn add_processed(&self, frames: u64) {
        self.processed_frames.fetch_add(frames, Ordering::Relaxed);
    }

    pub fn add_underrun(&self, samples: u64) {
        self.underrun_samples.fetch_add(samples, Ordering::Relaxed);
    }

    pub fn add_overrun(&self, samples: u64) {
        self.overrun_samples.fetch_add(samples, Ordering::Relaxed);
    }

    pub fn record_trim(&self) {
        self.trim_events.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_buffer_frames(&self, frames: u64) {
        self.buffer_frames.store(frames, Ordering::Relaxed);
    }

    pub fn set_ring_occupancy(&self, samples: u64) {
        self.ring_occupancy.store(samples, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            captured_frames: self.captured_frames(),
            processed_frames: self.processed_frames(),
            underrun_samples: self.underrun_samples(),
            overrun_samples: self.overrun_samples(),
            trim_events: self.trim_events(),
            buffer_frames: self.buffer_frames(),
            ring_occupancy: self.ring_occupancy(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Meters {
    input_peak: [AtomicU64; 2],
    input_rms: [AtomicU64; 2],
    output_peak: [AtomicU64; 2],
    output_rms: [AtomicU64; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterSnapshot {
    pub input_peak_db: [f32; 2],
    pub input_rms_db: [f32; 2],
    pub output_peak_db: [f32; 2],
    pub output_rms_db: [f32; 2],
}

impl Default for MeterSnapshot {
    fn default() -> Self {
        Self {
            input_peak_db: [f32::NEG_INFINITY; 2],
            input_rms_db: [f32::NEG_INFINITY; 2],
            output_peak_db: [f32::NEG_INFINITY; 2],
            output_rms_db: [f32::NEG_INFINITY; 2],
        }
    }
}

impl Meters {
    pub fn publish_input(&self, peak: [f32; 2], rms: [f32; 2]) {
        for channel in 0..2 {
            self.input_peak[channel].store(bits(peak[channel]), Ordering::Relaxed);
            self.input_rms[channel].store(bits(rms[channel]), Ordering::Relaxed);
        }
    }

    pub fn publish_output(&self, peak: [f32; 2], rms: [f32; 2]) {
        for channel in 0..2 {
            self.output_peak[channel].store(bits(peak[channel]), Ordering::Relaxed);
            self.output_rms[channel].store(bits(rms[channel]), Ordering::Relaxed);
        }
    }

    pub fn snapshot(&self) -> MeterSnapshot {
        MeterSnapshot {
            input_peak_db: read_db(&self.input_peak),
            input_rms_db: read_db(&self.input_rms),
            output_peak_db: read_db(&self.output_peak),
            output_rms_db: read_db(&self.output_rms),
        }
    }
}

fn bits(db: f32) -> u64 {
    u64::from(db.to_bits())
}

fn read_db(values: &[AtomicU64; 2]) -> [f32; 2] {
    [
        f32::from_bits(values[0].load(Ordering::Relaxed) as u32),
        f32::from_bits(values[1].load(Ordering::Relaxed) as u32),
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub kind: PipelineKind,
    pub sample_rate: u32,
    pub channels: usize,
    pub devices: Vec<DeviceInfo>,
    pub target: String,
    pub node_name: String,
    pub node_id: u32,
    pub node_description: String,
    pub chain: Vec<(&'static str, EffectSlot)>,
    pub bypassed: bool,
    pub automatic: bool,
    pub warnings: Vec<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            kind: PipelineKind::Output,
            sample_rate: 0,
            channels: 2,
            devices: Vec::new(),
            target: "default".to_owned(),
            node_name: String::new(),
            node_id: 0,
            node_description: String::new(),
            chain: Vec::new(),
            bypassed: false,
            automatic: true,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Shared {
    state: AtomicU8,
    error: Mutex<Option<String>>,
    stats: Stats,
    meters: Meters,
    snapshot: Mutex<Snapshot>,
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

impl Shared {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(EngineState::Stopped.code()),
            error: Mutex::new(None),
            stats: Stats::default(),
            meters: Meters::default(),
            snapshot: Mutex::new(Snapshot::default()),
        }
    }

    pub fn state(&self) -> EngineState {
        EngineState::from_code(self.state.load(Ordering::Relaxed))
    }

    pub fn set_state(&self, state: EngineState) {
        self.state.store(state.code(), Ordering::Relaxed);
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn set_error(&self, message: Option<String>) {
        if let Ok(mut guard) = self.error.lock() {
            *guard = message;
        }
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn meters(&self) -> &Meters {
        &self.meters
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .map_or_else(|_| Snapshot::default(), |guard| guard.clone())
    }

    pub fn update_snapshot<F: FnOnce(&mut Snapshot)>(&self, update: F) {
        if let Ok(mut guard) = self.snapshot.lock() {
            update(&mut guard);
        }
    }

    pub fn add_warning(&self, warning: String) {
        self.update_snapshot(|snapshot| {
            if !snapshot.warnings.contains(&warning) {
                snapshot.warnings.push(warning);
            }
            if snapshot.warnings.len() > 16 {
                snapshot.warnings.remove(0);
            }
        });
    }
}
