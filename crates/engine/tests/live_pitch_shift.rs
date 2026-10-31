mod common;

use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use pwe_core::ParamSet;
use pwe_engine::{BANDS, EffectSlot, EngineConfig, EngineHandle, EngineState, band_centre};

use common::{sink_target, wait_for, write_wav};

const MIN_PROCESSED_FRAMES: u64 = 4_800;
const TONE_HZ: f32 = 1_000.0;

fn loudest_band(hold: &[f32; BANDS]) -> f32 {
    let mut best = 0usize;
    for index in 1..BANDS {
        if hold[index] > hold[best] {
            best = index;
        }
    }
    band_centre(best)
}

fn accumulate(engine: &EngineHandle, hold: &mut [f32; BANDS], millis: u64) {
    let deadline = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < deadline {
        let spectrum = engine.shared().spectrum().snapshot();
        for (value, slot) in spectrum.iter().zip(hold.iter_mut()) {
            *slot = slot.max(*value);
        }
        sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "needs a running PipeWire daemon and plays a quiet tone"]
fn the_pitch_shift_stage_moves_the_tone_in_the_real_chain() {
    let _live = common::live_guard();
    let private = EngineConfig {
        node_name: format!("pipewire-effects-pitch-{}", std::process::id()),
        node_description: "PipeWire Effects pitch shift test".to_owned(),
        ..EngineConfig::default()
    };
    let mut engine = EngineHandle::spawn(private).expect("engine should start");
    assert!(
        wait_for(&engine, EngineState::Running, Duration::from_secs(5)),
        "engine did not start"
    );

    let params = ParamSet::new(pwe_dsp::effects::pitch_shift_specs());
    params.set_f32("semitones", 0.0).unwrap();
    engine
        .add_effect(
            EffectSlot(1),
            Box::new(pwe_dsp::effects::PitchShift::new(params.clone())),
        )
        .expect("effect should be accepted");
    sleep(Duration::from_millis(200));

    let target = sink_target(&engine);
    assert!(
        !target.is_empty(),
        "the virtual sink never appeared, so a client cannot be routed into it"
    );

    let path = std::env::temp_dir().join("pipewire-effects-pitch-live.wav");
    write_wav(&path, 6.0, TONE_HZ, 0.1).expect("the test signal should be written");

    let spawn = Command::new("pw-cat")
        .args(["--playback", "--target", target.as_str()])
        .arg(&path)
        .spawn();
    let mut child = match spawn {
        Ok(child) => child,
        Err(error) => {
            eprintln!("pw-cat is unavailable; the pitch shift test was not executed: {error}");
            let _ = std::fs::remove_file(&path);
            return;
        }
    };

    let mut tone_input_db: f32 = f32::MIN;
    let mut flat = [f32::NEG_INFINITY; BANDS];
    let deadline = Instant::now() + Duration::from_millis(1_800);
    while Instant::now() < deadline {
        let meters = engine.shared().meters().snapshot();
        if meters.input_peak_db[0] > tone_input_db {
            tone_input_db = meters.input_peak_db[0];
        }
        let spectrum = engine.shared().spectrum().snapshot();
        for (value, slot) in spectrum.iter().zip(flat.iter_mut()) {
            *slot = slot.max(*value);
        }
        sleep(Duration::from_millis(20));
    }

    params.set_f32("semitones", 12.0).unwrap();
    sleep(Duration::from_millis(400));
    let mut shifted = [f32::NEG_INFINITY; BANDS];
    accumulate(&engine, &mut shifted, 1_400);

    let _ = child.kill();
    let _ = child.wait();
    let stats = engine.shared().stats().snapshot();
    let _ = std::fs::remove_file(&path);

    let before = loudest_band(&flat);
    let after = loudest_band(&shifted);
    println!(
        "a {TONE_HZ:.0} Hz tone reached the chain at {tone_input_db:.2} dB; its loudest band read {before:.0} Hz and became {after:.0} Hz"
    );
    println!(
        "captured {} frames, processed {} frames, underruns {}, overruns {}, trims {}",
        stats.captured_frames,
        stats.processed_frames,
        stats.underrun_samples,
        stats.overrun_samples,
        stats.trim_events
    );

    assert!(
        tone_input_db > -30.0,
        "the tone never reached the chain, so the sink is not passing audio"
    );
    assert!(
        (before - TONE_HZ).abs() < 300.0,
        "the unshifted tone read {before} Hz instead of about {TONE_HZ} Hz"
    );
    assert!(
        (after - 2.0 * TONE_HZ).abs() < 400.0,
        "the shifted tone read {after} Hz instead of about {} Hz",
        2.0 * TONE_HZ
    );
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
