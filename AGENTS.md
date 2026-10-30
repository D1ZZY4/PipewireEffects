# Agent instructions

These rules are binding for every automated or human change in this repository.
Read them before editing. They come from the project requirements, not from a
convention that may be relaxed for convenience.

## Product constraints

- The application is PipeWire Effects, written in Rust.
- PipeWire is the only audio backend, through native `pipewire`/SPA bindings.
  Do not add PulseAudio, JACK, ALSA playback, GStreamer, SDL or a parallel
  audio path.
- The interface is GTK4 with libadwaita, written in Rust. Do not add Qt, QML,
  Kirigami, Electron, a web view or another toolkit.
- `references/` is a read-only EasyEffects checkout. Never modify it.
- Every hand-maintained source file stays at or below 350 physical lines.
- Do not add narrative comments, doc comments, `TODO`, `FIXME` or commented-out
  code. Keep only legal notices and the minimum safety documentation required
  by `unsafe` blocks or mandatory lints.
- All user-facing text goes through gettext (`pwe_core::i18n`). Regenerate
  `po/pipewire-effects.pot` with `scripts/update-po.sh` after adding strings.

## Processing backend is a required feature

`Preferences` → `Audio processing` → `Processing backend` must keep offering
three real modes:

- `CPU` — the whole chain on the audio thread.
- `GPU` — the elementwise gain stage on a Vulkan compute device; other effects
  stay on the CPU.
- `Both` — the same GPU stage plus a per-block CPU reference, replacing a
  result that differs by more than `max(1e-5, 1e-5 · |reference|)`. This is the
  built-in default; a machine with no Vulkan device falls back to `CPU` with a
  warning.

This setting is persisted as `audio_backend`, applies to both pipelines, and
restarts the running engines when it changes. It must never become a UI-only
switch, a mock, or a hidden no-op. If a change touches it:

- keep the Vulkan pipeline, buffers and warm-up outside the realtime callback;
- keep the fence wait bounded by half the block duration (100 µs floor, 25 ms
  ceiling); a miss must apply the gain on the CPU and increment the fallback
  counter;
- keep the three-consecutive-miss/Vulkan-error latch that returns the engine to
  the CPU;
- keep the absence of a Vulkan device a reported warning with a CPU fallback,
  never a crash and never a fake success.

The authoritative description, invariants and measurements are in
`docs/architecture.md`, section "Processing backend selection (CPU, GPU, Both)".
Do not delete or weaken that section, the CPU reference comparison, the
diagnostics counters, or the two live tests in
`crates/engine/tests/live_gpu.rs`.

## Verification

Run before claiming a change is complete:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Against a running PipeWire daemon, also run the integration tests explicitly:

```sh
cargo test --release -p pwe-engine --test live_pipewire -- --ignored --nocapture --test-threads=1
cargo test --release -p pwe-engine --test live_gpu -- --ignored --nocapture --test-threads=1
```

Interface changes require new captures. Use `scripts/capture-ui.sh` (see
`scripts/README.md`), inspect the PNGs, and record the result in
`docs/architecture.md`. The harness never changes the user's settings, default
device or audio services; keep it that way.

The virtual sink and source must never become the user's default route. After a
capture or live-test session, check that the session default still names a real
device:

```sh
pw-metadata -n default | grep default.audio.sink
```

If it names `pipewire-effects-sink` or `pipewire-effects-source` because an
earlier run left it there, restore the real device with `wpctl set-default
<real-node-id>` (or `wpctl status` to find the id). Never leave the app's own
nodes as the default, and never write the user's WirePlumber state by hand
unless you are undoing the app's own footprint.

A compile, a single screenshot or a unit test is not proof that a UI control or
an audio effect works. Distinguish verified facts from assumptions in reports.

## Documentation

Keep these current with the code, not with intentions:

- `docs/architecture.md` — actual structure, audio path, realtime rules,
  backend decision, interface, verification.
- `docs/feature-matrix.md` — every feature with status and evidence.
- `docs/reference-audit.md` — reference revision, scope, licence findings.

Do not claim EasyEffects parity, performance improvements or verification that
did not happen. Record blockers precisely.