use std::f32::consts::TAU;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use pwe_core::ParamSet;
use pwe_engine::{EffectSlot, EngineConfig, EngineHandle, EngineState};

const MIN_PROCESSED_FRAMES: u64 = 4_800;

fn write_wav(path: &PathBuf, seconds: f32, frequency: f32, amplitude: f32) -> std::io::Result<()> {
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

fn wait_for(engine: &EngineHandle, state: EngineState, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if engine.shared().state() == state {
            return true;
        }
        sleep(Duration::from_millis(25));
    }
    engine.shared().state() == state
}

#[test]
#[ignore = "needs a running PipeWire daemon"]
fn the_engine_connects_and_reports_its_devices() {
    let mut engine = EngineHandle::spawn(EngineConfig::default()).expect("engine should start");
    assert!(
        wait_for(&engine, EngineState::Running, Duration::from_secs(5)),
        "engine did not start"
    );
    let snapshot = engine.shared().snapshot();
    assert!(!snapshot.devices.is_empty(), "no devices were discovered");
    assert!(snapshot.devices.iter().any(|device| device.is_sink()));
    assert_eq!(snapshot.channels, 2);
    assert_eq!(snapshot.sink_node_name, "pipewire-effects-sink");
    engine.shutdown();
    assert_eq!(engine.shared().state(), EngineState::Stopped);
}

#[test]
#[ignore = "needs a running PipeWire daemon and plays a quiet tone"]
fn audio_is_processed_by_the_chain_and_measured_at_the_output() {
    let private = EngineConfig {
        sink_node_name: format!("pipewire-effects-live-{}", std::process::id()),
        sink_description: "PipeWire Effects live test".to_owned(),
        ..EngineConfig::default()
    };
    let mut engine = EngineHandle::spawn(private).expect("engine should start");
    assert!(
        wait_for(&engine, EngineState::Running, Duration::from_secs(5)),
        "engine did not start"
    );

    let params = ParamSet::new(pwe_dsp::effects::gain_specs());
    params.set_f32("gain", -20.0).unwrap();
    engine
        .add_effect(EffectSlot(1), Box::new(pwe_dsp::effects::Gain::new(params)))
        .expect("effect should be accepted");
    sleep(Duration::from_millis(200));

    let sink_deadline = Instant::now() + Duration::from_secs(5);
    let mut sink_target = String::new();
    while Instant::now() < sink_deadline {
        let snapshot = engine.shared().snapshot();
        if snapshot.sink_node_id != 0
            && let Some(device) = snapshot
                .devices
                .iter()
                .find(|device| device.id == snapshot.sink_node_id)
        {
            sink_target = if device.serial != 0 {
                device.serial.to_string()
            } else {
                device.name.clone()
            };
            break;
        }
        sleep(Duration::from_millis(50));
    }
    assert!(
        !sink_target.is_empty(),
        "the virtual sink never appeared, so a client cannot be routed into it"
    );

    let path = std::env::temp_dir().join("pipewire-effects-live-test.wav");
    write_wav(&path, 2.5, 1_000.0, 0.1).expect("the test signal should be written");

    let mut child: Option<std::process::Child> = None;
    let mut tone_input_db = f32::MIN;
    let mut reduction = 0.0f32;
    let mut samples = 0u32;
    let mut attempts = 0u32;
    while attempts < 3 && tone_input_db <= -30.0 {
        attempts += 1;
        match Command::new("pw-cat")
            .args(["--playback", "--target", sink_target.as_str()])
            .arg(&path)
            .spawn()
        {
            Ok(spawned) => child = Some(spawned),
            Err(error) => {
                eprintln!(
                    "pw-cat is unavailable; the live processing test was not executed: {error}"
                );
                let _ = std::fs::remove_file(&path);
                return;
            }
        }
        let attempt_deadline = Instant::now() + Duration::from_secs(4);
        let mut heard = false;
        while Instant::now() < attempt_deadline {
            let meters = engine.shared().meters().snapshot();
            let input = meters.input_peak_db[0];
            let output = meters.output_peak_db[0];
            samples += 1;
            if input > -30.0 {
                heard = true;
                if output > -100.0 {
                    let measured = input - output;
                    if measured > 10.0 && measured < 60.0 {
                        tone_input_db = input;
                        reduction = measured;
                        if engine.shared().stats().snapshot().processed_frames
                            >= MIN_PROCESSED_FRAMES
                        {
                            break;
                        }
                    }
                }
            }
            sleep(Duration::from_millis(20));
        }
        if heard && tone_input_db > -30.0 {
            break;
        }
        if !heard {
            if let Some(mut running) = child.take() {
                let _ = running.kill();
                let _ = running.wait();
            }
            sleep(Duration::from_millis(300));
        } else {
            break;
        }
    }
    if let Some(mut running) = child.take() {
        let _ = running.kill();
        let _ = running.wait();
    }
    let meters = engine.shared().meters().snapshot();
    let stats = engine.shared().stats().snapshot();

    let _ = std::fs::remove_file(&path);

    println!(
        "attempts {}; sampled {} times; captured {} frames, processed {} frames, underruns {}, overruns {}, trims {}, occupancy {}",
        attempts,
        samples,
        stats.captured_frames,
        stats.processed_frames,
        stats.underrun_samples,
        stats.overrun_samples,
        stats.trim_events,
        stats.ring_occupancy
    );
    println!(
        "tone measured at {tone_input_db:.2} dB with {reduction:.2} dB of reduction; last read: input peak {:.2} dB, output peak {:.2} dB",
        meters.input_peak_db[0], meters.output_peak_db[0]
    );

    assert!(
        stats.captured_frames >= MIN_PROCESSED_FRAMES,
        "the capture stream saw no audio"
    );
    assert!(
        stats.processed_frames >= MIN_PROCESSED_FRAMES,
        "the chain never ran"
    );
    assert!(tone_input_db > -30.0, "the input meter never moved");
    assert!(
        (tone_input_db + 20.0).abs() < 3.0,
        "the tone reached the chain at {tone_input_db} dB instead of about -20 dB, so the sink or the source stream is not passing audio at unity"
    );
    assert!(
        (reduction - 20.0).abs() < 4.0,
        "expected about 20 dB of gain reduction, measured {reduction}"
    );

    engine.shutdown();
}

#[test]
#[ignore = "needs a running PipeWire daemon"]
fn the_target_device_can_be_changed_while_running() {
    let mut engine = EngineHandle::spawn(EngineConfig::default()).expect("engine should start");
    assert!(
        wait_for(&engine, EngineState::Running, Duration::from_secs(5)),
        "engine did not start"
    );

    let devices = engine.shared().snapshot().devices;
    let Some(sink) = devices.iter().find(|device| device.is_sink()) else {
        eprintln!("no sink is available; the retarget test was not executed");
        return;
    };
    let name = sink.name.clone();
    engine
        .set_target_device(name.clone())
        .expect("the command should be queued");
    sleep(Duration::from_millis(500));
    assert_eq!(engine.shared().snapshot().target, name);
    assert!(wait_for(
        &engine,
        EngineState::Running,
        Duration::from_secs(3)
    ));

    engine
        .set_target_device("does-not-exist")
        .expect("the command should be queued");
    sleep(Duration::from_millis(500));
    assert!(
        engine
            .shared()
            .snapshot()
            .warnings
            .iter()
            .any(|warning| warning.contains("does-not-exist")),
        "the missing device was not reported"
    );
    engine.shutdown();
}
