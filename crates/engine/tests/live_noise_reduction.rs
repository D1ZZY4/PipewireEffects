mod common;

use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use pwe_core::ParamSet;
use pwe_engine::{EffectSlot, EngineConfig, EngineHandle, EngineState};

use common::{sink_target, wait_for, write_wav};

const MIN_PROCESSED_FRAMES: u64 = 4_800;

#[test]
#[ignore = "needs a running PipeWire daemon and plays a quiet tone"]
fn the_noise_reduction_stage_runs_inside_the_realtime_callback() {
    let _live = common::live_guard();
    let private = EngineConfig {
        node_name: format!("pipewire-effects-denoise-{}", std::process::id()),
        node_description: "PipeWire Effects noise reduction test".to_owned(),
        ..EngineConfig::default()
    };
    let mut engine = EngineHandle::spawn(private).expect("engine should start");
    assert!(
        wait_for(&engine, EngineState::Running, Duration::from_secs(5)),
        "engine did not start"
    );

    let params = ParamSet::new(pwe_dsp::effects::noise_reduction_specs());
    params.set_f32("mix", 100.0).unwrap();
    params.set_bool("vad", false).unwrap();
    engine
        .add_effect(
            EffectSlot(1),
            Box::new(pwe_dsp::effects::NoiseReduction::new(params)),
        )
        .expect("effect should be accepted");
    sleep(Duration::from_millis(200));

    let target = sink_target(&engine);
    assert!(
        !target.is_empty(),
        "the virtual sink never appeared, so a client cannot be routed into it"
    );

    let path = std::env::temp_dir().join("pipewire-effects-denoise-live.wav");
    write_wav(&path, 2.5, 1_000.0, 0.1).expect("the test signal should be written");

    let mut child: Option<std::process::Child> = None;
    let mut tone_input_db = f32::MIN;
    let mut reduction = 0.0f32;
    let mut attempts = 0u32;
    while attempts < 3 && tone_input_db <= -30.0 {
        attempts += 1;
        match Command::new("pw-cat")
            .args(["--playback", "--target", target.as_str()])
            .arg(&path)
            .spawn()
        {
            Ok(spawned) => child = Some(spawned),
            Err(error) => {
                eprintln!("pw-cat is unavailable; the denoise test was not executed: {error}");
                let _ = std::fs::remove_file(&path);
                return;
            }
        }
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            let meters = engine.shared().meters().snapshot();
            let input = meters.input_peak_db[0];
            let output = meters.output_peak_db[0];
            if input > -30.0 && output.is_finite() {
                if tone_input_db <= -30.0 {
                    tone_input_db = input;
                }
                reduction = reduction.max(input - output);
            }
            sleep(Duration::from_millis(20));
        }
    }
    if let Some(mut running) = child.take() {
        let _ = running.kill();
        let _ = running.wait();
    }
    let stats = engine.shared().stats().snapshot();
    let _ = std::fs::remove_file(&path);

    println!(
        "attempts {attempts}; captured {} frames, processed {} frames, underruns {}, overruns {}, trims {}",
        stats.captured_frames,
        stats.processed_frames,
        stats.underrun_samples,
        stats.overrun_samples,
        stats.trim_events
    );
    println!(
        "a 1 kHz tone reached the chain at {tone_input_db:.2} dB and left it {reduction:.2} dB lower"
    );

    assert!(
        (tone_input_db + 20.0).abs() < 3.0,
        "the tone reached the chain at {tone_input_db} dB instead of about -20 dB, so the sink is not passing audio at unity"
    );
    assert!(
        stats.processed_frames >= MIN_PROCESSED_FRAMES,
        "the chain never ran"
    );
    assert!(
        reduction > 20.0,
        "the network left the tone almost untouched, at {reduction} dB of reduction"
    );
    assert!(
        stats.underrun_samples < stats.processed_frames / 8,
        "the callback missed {} samples of {}, so the stage is not keeping up",
        stats.underrun_samples,
        stats.processed_frames
    );

    engine.shutdown();
}
