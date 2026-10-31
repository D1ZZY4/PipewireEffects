mod common;

use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use pwe_core::ParamSet;
use pwe_engine::{EffectSlot, EngineConfig, EngineHandle, EngineState};

use common::{sink_target, wait_for, write_wav};

const MIN_PROCESSED_FRAMES: u64 = 4_800;
const TONE_HZ: f32 = 60.0;

#[test]
#[ignore = "needs a running PipeWire daemon and plays a quiet tone"]
fn the_bass_loudness_stage_lifts_a_low_tone_in_the_real_chain() {
    let _live = common::live_guard();
    let private = EngineConfig {
        node_name: format!("pipewire-effects-loudness-{}", std::process::id()),
        node_description: "PipeWire Effects loudness test".to_owned(),
        ..EngineConfig::default()
    };
    let mut engine = EngineHandle::spawn(private).expect("engine should start");
    assert!(
        wait_for(&engine, EngineState::Running, Duration::from_secs(5)),
        "engine did not start"
    );

    let params = ParamSet::new(pwe_dsp::effects::bass_loudness_specs());
    params.set_f32("loudness", -50.0).unwrap();
    params.set_f32("output", 0.0).unwrap();
    params.set_f32("link", 0.0).unwrap();
    engine
        .add_effect(
            EffectSlot(1),
            Box::new(pwe_dsp::effects::BassLoudness::new(params)),
        )
        .expect("effect should be accepted");
    sleep(Duration::from_millis(200));

    let target = sink_target(&engine);
    assert!(
        !target.is_empty(),
        "the virtual sink never appeared, so a client cannot be routed into it"
    );

    let path = std::env::temp_dir().join("pipewire-effects-loudness-live.wav");
    write_wav(&path, 3.0, TONE_HZ, 0.1).expect("the test signal should be written");

    let spawn = Command::new("pw-cat")
        .args(["--playback", "--target", target.as_str()])
        .arg(&path)
        .spawn();
    let mut child = match spawn {
        Ok(child) => child,
        Err(error) => {
            eprintln!("pw-cat is unavailable; the loudness test was not executed: {error}");
            let _ = std::fs::remove_file(&path);
            return;
        }
    };

    let mut input_db: f32 = f32::MIN;
    let mut output_db: f32 = f32::MIN;
    let deadline = Instant::now() + Duration::from_millis(2_200);
    while Instant::now() < deadline {
        let meters = engine.shared().meters().snapshot();
        if meters.input_peak_db[0] > input_db {
            input_db = meters.input_peak_db[0];
        }
        if meters.output_peak_db[0] > output_db {
            output_db = meters.output_peak_db[0];
        }
        sleep(Duration::from_millis(20));
    }

    let _ = child.kill();
    let _ = child.wait();
    let stats = engine.shared().stats().snapshot();
    let _ = std::fs::remove_file(&path);

    let lift = output_db - input_db;
    println!(
        "a {TONE_HZ:.0} Hz tone arrived at {input_db:.2} dB and left at {output_db:.2} dB, a lift of {lift:.2} dB"
    );
    println!(
        "captured {} frames, processed {} frames, underruns {}, overruns {}, trims {}",
        stats.captured_frames,
        stats.processed_frames,
        stats.underrun_samples,
        stats.overrun_samples,
        stats.trim_events
    );

    assert!(input_db > -30.0, "the tone never reached the chain");
    assert!(lift > 3.0, "the contour lifted the tone by only {lift} dB");
    assert!(
        stats.processed_frames >= MIN_PROCESSED_FRAMES,
        "the chain never ran"
    );
    assert!(
        stats.underrun_samples < stats.processed_frames / 8,
        "the callback missed {} samples of {}, so the stage is not keeping up",
        stats.underrun_samples,
        stats.processed_frames
    );

    engine.shutdown();
}
