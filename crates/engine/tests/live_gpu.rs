mod common;

use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use pwe_core::ParamSet;
use pwe_engine::gpu::{self, AudioBackend};
use pwe_engine::{EffectSlot, EngineConfig, EngineHandle, EngineState, StatsSnapshot};

use common::{sink_target, wait_for, write_wav};

const MIN_PROCESSED_FRAMES: u64 = 4_800;

fn run_with_backend(backend: AudioBackend) -> Option<StatsSnapshot> {
    let _live = common::live_guard();
    if !gpu::available() {
        eprintln!("no Vulkan device is available; the backend test was not executed");
        return None;
    }
    let private = EngineConfig {
        node_name: format!(
            "pipewire-effects-gpu-{}-{}",
            backend.key(),
            std::process::id()
        ),
        node_description: "PipeWire Effects backend test".to_owned(),
        backend,
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

    let target = sink_target(&engine);
    assert!(
        !target.is_empty(),
        "the virtual sink never appeared, so a client cannot be routed into it"
    );

    let path = std::env::temp_dir().join(format!(
        "pipewire-effects-gpu-test-{}-{}.wav",
        backend.key(),
        std::process::id()
    ));
    write_wav(&path, 2.5, 1_000.0, 0.1).expect("the test signal should be written");

    let mut child: Option<std::process::Child> = None;
    let mut heard = false;
    for _ in 0..3 {
        match Command::new("pw-cat")
            .args(["--playback", "--target", target.as_str()])
            .arg(&path)
            .spawn()
        {
            Ok(spawned) => child = Some(spawned),
            Err(error) => {
                eprintln!("pw-cat is unavailable; the backend test was not executed: {error}");
                let _ = std::fs::remove_file(&path);
                engine.shutdown();
                return None;
            }
        }
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            if engine.shared().meters().snapshot().input_peak_db[0] > -30.0 {
                heard = true;
                break;
            }
            sleep(Duration::from_millis(20));
        }
        if heard && engine.shared().stats().snapshot().processed_frames >= MIN_PROCESSED_FRAMES {
            break;
        }
        if let Some(mut running) = child.take() {
            let _ = running.kill();
            let _ = running.wait();
        }
        sleep(Duration::from_millis(300));
    }
    if let Some(mut running) = child.take() {
        let _ = running.kill();
        let _ = running.wait();
    }
    let stats = engine.shared().stats().snapshot();
    let _ = std::fs::remove_file(&path);
    println!(
        "backend {}: gpu blocks {}, fallbacks {}, disagreements {}, processed {} frames",
        backend.key(),
        stats.gpu_blocks,
        stats.gpu_fallbacks,
        stats.gpu_disagreements,
        stats.processed_frames
    );
    engine.shutdown();
    Some(stats)
}

fn assert_gpu_ran(stats: &StatsSnapshot) {
    assert!(
        stats.processed_frames >= MIN_PROCESSED_FRAMES,
        "the chain never ran"
    );
    assert!(
        stats.gpu_blocks > 0,
        "the GPU stage never ran while a GPU backend was selected"
    );
    assert!(
        stats.gpu_fallbacks * 10 <= stats.gpu_blocks,
        "the GPU stage missed its deadline too often: {} fallbacks for {} blocks",
        stats.gpu_fallbacks,
        stats.gpu_blocks
    );
}

#[test]
#[ignore = "needs a running PipeWire daemon and a Vulkan compute device"]
fn the_gpu_backend_computes_the_gain_stage_on_the_device() {
    if let Some(stats) = run_with_backend(AudioBackend::Gpu) {
        assert_gpu_ran(&stats);
    }
}

#[test]
#[ignore = "needs a running PipeWire daemon and a Vulkan compute device"]
fn the_both_backend_verifies_the_gpu_against_the_cpu() {
    if let Some(stats) = run_with_backend(AudioBackend::Both) {
        assert_gpu_ran(&stats);
        assert_eq!(
            stats.gpu_disagreements, 0,
            "the GPU result disagreed with the CPU reference"
        );
    }
}
