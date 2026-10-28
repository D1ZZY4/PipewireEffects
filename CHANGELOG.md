# Changelog

Notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The preset file format carries its own `schema_version`, which is tracked
separately from the application version so that a preset written by one release
keeps working in the next.

## [Unreleased]

## [0.1.0] - 2026-10-28

The first working version: an output pipeline, three effects, and a preset
browser.

### Added

- A virtual PipeWire sink that any application can be routed to, built from two
  `pw_stream`s joined into one node group, with a lock-free ring buffer between
  them.
- **Parametric Equalizer**: eight bands, each with a shape (peak, low shelf,
  high shelf, high pass, low pass, notch, band pass, all pass), frequency, gain
  and Q, plus a pre-amplifier. 41 parameters.
- **Limiter**: a brick-wall peak limiter with a ceiling, make-up gain, release
  time and an optional linked stereo detector.
- **Output Gain**: a constant gain stage with a mute.
- A chain you can reorder, enable and bypass per effect, or bypass as a whole.
- Presets: five built in, stored as readable JSON, with a preset browser and
  save and delete in the interface.
- Live input and output level meters, with peak hold.
- Counters for underruns, overruns, latency trims and dropped effects, shown in
  the interface.
- An output device chooser, defaulting to whatever the system is using.
- A command line: `--list-effects`, `--list-devices`, `--device` and
  `--latency`.
- A desktop file and an application icon.

### Design notes

Two properties are worth recording, because later changes should preserve them:

- The interface renders whatever parameter specifications an effect declares, so
  adding an effect makes it fully controllable with no changes to the interface.
- The audio thread never allocates, locks or frees. Parameter changes travel
  through atomics, structural edits through a lock-free queue, and removed
  effects are recycled on a non-realtime thread.

### Verified

- 188 tests that need neither a display nor an audio device.
- Three live tests against a running PipeWire daemon, feeding a tone into the
  virtual sink with `pw-cat` and checking the level leaving the chain:
  `input peak 0.5000, output peak 0.0500 (expected 0.0500), glitches 0`.

### Known limitations

- Only the output pipeline exists. Input (microphone) processing is not
  implemented; the preset schema has room for it.
- Changing the output device restarts the audio engine.
- Drift between the capture and playback streams is corrected by trimming the
  buffer rather than by resampling.
- The limiter has no lookahead, so its instantaneous attack trades some
  transient distortion for a guaranteed ceiling.

[Unreleased]: https://github.com/D1ZZY4/PipewireEffects/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/D1ZZY4/PipewireEffects/releases/tag/v0.1.0
