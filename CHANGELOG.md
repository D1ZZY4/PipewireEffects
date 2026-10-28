# Changelog

Notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The preset file format carries its own `schema_version`, which is tracked
separately from the application version so that a preset written by one release
keeps working in the next.

## [Unreleased]

## [0.1.0] - 2026-10-28

The first complete implementation: an output pipeline, thirteen effects, a
generated control surface and a preset library.

### Added

- A virtual PipeWire sink (`pipewire-effects-sink`) built from two `pw_stream`s
  in one node group, with a lock-free single-producer/single-consumer ring
  buffer between them, silence padding on underrun, newest-sample drops on
  overrun and drift trimming on the consumer side.
- Thirteen effects, all implemented in Rust: equalizer, filter, compressor,
  limiter, gate, de-esser, auto gain, bass enhancer, crystalizer, stereo tools,
  gain, delay and reverberation.
- A parameter model in which effects publish metadata (identifier, label, type,
  range, unit, grouping) and keep live values in atomics shared with the audio
  thread. The interface renders any effect from its metadata alone.
- A chain that supports adding, removing, reordering, per-stage enable and
  global bypass, mirrored to the audio thread through a fixed-capacity lock-free
  queue.
- Presets: six built in, JSON on disk, with save, delete, import, export, value
  coercion, name validation, atomic writes and non-fatal reporting of unknown
  content.
- A GTK4/libadwaita interface: navigation split view, parameter editors grouped
  by section, an analytic equalizer response curve drawn with cairo, effect
  picker with search, preset browser, preferences dialog with device selection
  and diagnostics.
- Live input and output meters with peak and RMS, and counters for underruns,
  overruns and drift trims.
- A desktop file and an application icon.

### Design notes

Two properties should survive later changes:

- **The interface is generic.** It renders whatever parameter specifications an
  effect declares, so a new effect needs no interface code.
- **The audio thread never allocates, locks or frees.** Parameter changes use
  atomics, structural edits use a lock-free queue, and removed effects are
  recycled on a non-realtime thread.

The audio path uses two streams rather than one `pw_filter` per effect because
`pipewire-rs` 0.10 does not bind `pw_filter`; the trade-off and the exit path are
recorded in `docs/architecture.md`.

### Verified

- 127 unit and integration tests that need neither a display nor an audio
  device.
- Three live tests against a running PipeWire daemon: a tone is fed into the
  virtual sink with `pw-cat` and the level leaving the chain is measured.
  A representative run captured and processed 54 272 frames, measured
  -20.00 dB at the input and -40.00 dB at the output for a -20 dB gain stage,
  and reported 4 096 underrun samples from the initial fill with no overruns
  and no trims.
- A window smoke test (`PIPEWIRE_EFFECTS_SMOKE_TEST=1`) that builds the real
  interface, adds an effect, loads a preset, and reports the engine state,
  device count and negotiated rate.

### Known limitations

- No input (microphone) pipeline and therefore no virtual source.
- Changing the output device rebuilds both streams.
- Drift is corrected by trimming, not resampling.
- The limiter has no lookahead.
- Auto gain is RMS-based rather than EBU R128.
- The default output device is chosen from `priority.session`, not from
  `default.audio.sink` metadata.
- No plugin hosting, FFT convolution, spectrum analyser, neural noise
  suppression, pitch shifting or multiband processing.
- English only; no translations, autostart or packaging.

[Unreleased]: https://github.com/D1ZZY4/PipewireEffects/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/D1ZZY4/PipewireEffects/releases/tag/v0.1.0