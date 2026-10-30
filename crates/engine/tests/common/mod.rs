use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::sleep;
use std::time::{Duration, Instant};

use pwe_engine::{EngineHandle, EngineState};

static LIVE_GRAPH: Mutex<()> = Mutex::new(());

pub fn live_guard() -> MutexGuard<'static, ()> {
    LIVE_GRAPH.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn write_wav(
    path: &PathBuf,
    seconds: f32,
    frequency: f32,
    amplitude: f32,
) -> std::io::Result<()> {
    let rate = 48_000u32;
    let frames = (rate as f32 * seconds) as u32;
    let channels = 2u16;
    let mut bytes = Vec::with_capacity(44 + frames as usize * 8);
    let data_size = frames * u32::from(channels) * 4;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 4).to_le_bytes());
    bytes.extend_from_slice(&(channels * 4).to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for frame in 0..frames {
        let value = amplitude * (TAU * frequency * frame as f32 / rate as f32).sin();
        for _ in 0..channels {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    std::fs::write(path, bytes)
}

pub fn wait_for(engine: &EngineHandle, state: EngineState, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if engine.shared().state() == state {
            return true;
        }
        sleep(Duration::from_millis(25));
    }
    engine.shared().state() == state
}

pub fn sink_target(engine: &EngineHandle) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let snapshot = engine.shared().snapshot();
        if snapshot.node_id != 0
            && let Some(device) = snapshot
                .devices
                .iter()
                .find(|device| device.id == snapshot.node_id)
        {
            return if device.serial != 0 {
                device.serial.to_string()
            } else {
                device.name.clone()
            };
        }
        sleep(Duration::from_millis(50));
    }
    String::new()
}
