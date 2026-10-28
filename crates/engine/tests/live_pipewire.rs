//! End-to-end checks against a running PipeWire daemon.
//!
//! These are `#[ignore]`d by default. They need a real PipeWire session, they
//! take a few seconds each, and they briefly play a quiet tone on whatever the
//! system's default output happens to be.
//!
//! ```text
//! cargo test -p pipewire-effects-engine --test live_pipewire -- --ignored --nocapture
//! ```
//!
//! # What they prove that the unit tests cannot
//!
//! The unit tests cover the DSP and the ring buffer in isolation. These tests
//! close the remaining gap: that a *real client application* — `pw-cat`, the
//! same tool a user would use — playing into our virtual sink is actually
//! routed through the effect chain and measured on the far side at the level
//! the effect settings say it should be.
//!
//! The measurement is taken from the engine's own output meters, which sit
//! after the chain and before the device. That makes the assertion meaningful:
//! if the audio never reached the chain, or the chain did not run, or the
//! engine was measuring the wrong buffer, the level would not come out where it
//! should.

use std::f32::consts::PI;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use pipewire_effects_core::{ChainController, ParamValue};
use pipewire_effects_dsp::registry;
use pipewire_effects_engine::{Engine, EngineConfig, ignore_events, list_devices, list_sinks};

/// Amplitude of the test tone, well below full scale.
const TONE_AMPLITUDE: f32 = 0.5;
/// How long the tone plays.
const TONE_SECONDS: f32 = 0.6;
/// The gain the chain applies, in decibels.
const APPLIED_GAIN_DB: f32 = -20.0;
/// How long to wait for the graph to come up.
const STARTUP: Duration = Duration::from_secs(3);

/// A configuration with unique node names, so repeated runs do not collide.
fn config(tag: &str) -> EngineConfig {
    EngineConfig {
        capture_node_name: format!("pipewire-effects-sink-{}-{tag}", std::process::id()),
        playback_node_name: format!("pipewire-effects-out-{}-{tag}", std::process::id()),
        ..EngineConfig::default()
    }
}

/// Waits until the named node appears as an audio device.
fn wait_for_device(name: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(devices) = list_devices(Duration::from_millis(300)) {
            if devices.iter().any(|device| device.name == name) {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Streams a raw `f32` tone into a node with `pw-cat`.
///
/// The writes block once `pw-cat`'s buffer is full, which paces the tone at
/// roughly real time — so sampling the meters after each chunk catches the
/// signal rather than racing past it.
fn play_tone(target: &str, on_chunk: &mut dyn FnMut()) -> std::io::Result<()> {
    let mut child = Command::new("pw-cat")
        .args([
            "--playback",
            "--raw",
            "--format",
            "f32",
            "--rate",
            "48000",
            "--channels",
            "2",
            "--target",
            target,
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin was piped");

    // Ten milliseconds per chunk, matching a typical PipeWire quantum.
    const CHUNK_FRAMES: usize = 480;
    let chunks = ((TONE_SECONDS * 48_000.0) as usize) / CHUNK_FRAMES;
    let mut phase = 0.0_f32;

    for _ in 0..chunks {
        let mut bytes = Vec::with_capacity(CHUNK_FRAMES * 2 * 4);
        for _ in 0..CHUNK_FRAMES {
            let value = phase.sin() * TONE_AMPLITUDE;
            phase = (phase + 2.0 * PI * 440.0 / 48_000.0) % (2.0 * PI);
            bytes.extend_from_slice(&value.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        if stdin.write_all(&bytes).is_err() {
            break;
        }
        let _ = stdin.flush();
        on_chunk();
        // Pacing: without it we would race ahead of real time and finish the
        // tone before the engine had processed any of it.
        thread::sleep(Duration::from_millis(4));
    }

    drop(stdin);
    let _ = child.wait();
    Ok(())
}

#[test]
#[ignore = "needs a running PipeWire daemon"]
fn the_virtual_sink_is_registered_as_a_real_audio_sink() {
    let config = config("register");
    let node = config.capture_node_name.clone();
    let controller = ChainController::new(registry());

    let (mut engine, _handles) = Engine::spawn(config, controller.command_queue(), ignore_events())
        .expect("the engine should start against a running PipeWire daemon");

    let found = wait_for_device(&node, STARTUP);
    engine.stop();

    assert!(
        found,
        "`{node}` never appeared as an audio device, so nothing can be routed to it"
    );
}

#[test]
#[ignore = "needs a running PipeWire daemon"]
fn audio_flows_from_a_client_through_the_chain_at_the_expected_level() {
    let controller = ChainController::new(registry());

    // A single gain stage, so the expected output level is arithmetic rather
    // than a matter of taste.
    controller.add("gain").expect("gain is registered");
    controller.with_model(|model| {
        model.entries[0]
            .parameters
            .set_by_id("gain", ParamValue::Float(APPLIED_GAIN_DB))
            .expect("the gain parameter exists");
    });

    let config = config("audio");
    let node = config.capture_node_name.clone();
    let (mut engine, handles) = Engine::spawn(config, controller.command_queue(), ignore_events())
        .expect("the engine should start against a running PipeWire daemon");

    assert!(
        wait_for_device(&node, STARTUP),
        "`{node}` never appeared, so the test cannot feed it audio"
    );

    // Feed the tone and watch the meters while it plays.
    let mut loudest_output = 0.0_f32;
    let mut loudest_input = 0.0_f32;
    play_tone(&node, &mut || {
        loudest_input = loudest_input.max(handles.input_meters().snapshot().loudest_peak());
        loudest_output = loudest_output.max(handles.output_meters().snapshot().loudest_peak());
    })
    .expect("pw-cat should run");

    let stats = handles.stats();
    let latency_ms = stats.latency_ms(48_000, 2);
    engine.stop();

    // The client's audio must have arrived.
    assert!(
        stats.capture_frames > 0,
        "no audio reached the virtual sink: {stats:?}"
    );
    assert!(
        stats.processed_frames > 0,
        "nothing was processed or played: {stats:?}"
    );
    assert!(
        loudest_input > 0.05,
        "the input meter never showed the tone, peak was {loudest_input}"
    );

    // And it must have come out of the chain at the level the effect asked for.
    let expected = TONE_AMPLITUDE * 10.0_f32.powf(APPLIED_GAIN_DB / 20.0);
    assert!(
        loudest_output > expected * 0.5 && loudest_output < expected * 1.6,
        "expected about {expected} on the output meter after {APPLIED_GAIN_DB} dB of gain, \
         but measured {loudest_output}"
    );

    println!(
        "captured {} frames, processed {} frames, buffer {:.1} ms, \
         input peak {loudest_input:.4}, output peak {loudest_output:.4} (expected {expected:.4}), \
         glitches {} ",
        stats.capture_frames,
        stats.processed_frames,
        latency_ms,
        stats.glitch_samples()
    );
}

#[test]
#[ignore = "needs a running PipeWire daemon"]
fn the_virtual_sink_is_listed_alongside_the_real_devices() {
    let config = config("listed");
    let node = config.capture_node_name.clone();
    let controller = ChainController::new(registry());

    let (mut engine, _handles) =
        Engine::spawn(config, controller.command_queue(), ignore_events()).unwrap();

    let appeared = wait_for_device(&node, STARTUP);
    // `list_sinks` deliberately excludes our own virtual sink, so it must be
    // absent there while being present in the full device list.
    let visible_as_output = list_sinks(Duration::from_millis(300))
        .map(|sinks| sinks.iter().any(|sink| sink.name == node))
        .unwrap_or(false);
    engine.stop();

    assert!(appeared, "the virtual sink was not registered");
    assert!(
        !visible_as_output,
        "`list_sinks` should hide our own virtual sink from the device chooser"
    );
}
