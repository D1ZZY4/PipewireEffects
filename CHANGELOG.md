# Changelog

Notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The preset file format carries its own `schema_version`, which is tracked
separately from the application version so that a preset written by one release
keeps working in the next.

## [Unreleased]

### Fixed

- The destination device now follows the system default sink. The engine reads
  `default.audio.sink` from the session manager's metadata before it creates its
  own sink, and re-resolves the destination when that value changes while it
  runs. Previously the output was chosen by a `priority.session` heuristic, so on
  a machine whose default is a Bluetooth headset the processed audio was sent to
  the built-in analogue output and nothing was heard. The *Devices* page gained a
  *System default* entry so the automatic choice can be restored after picking a
  device by hand.
- The input and output level meters now measure with the same ballistics. The
  output meter computed the raw block peak while the input meter ran a decaying
  peak hold, so an effect-free chain reported different levels on the two sides
  (a live −12 dBFS signal read as −26 dBFS at the output). Both meters share
  `LevelMeter` now, and a test asserts that an empty chain reports the same level
  at both.
- The application icon resolves when the program is run from a build directory.
  The icon is compiled into the binary as a `GResource` and registered with the
  icon theme, so the About dialog and the header bar no longer show the
  missing-image glyph on a machine where `data/icons` was never installed.
- Every icon the interface uses exists in the Adwaita icon theme, and the set is
  checked at startup and in a test. `emblem-music-symbolic` (the tone effects and
  the *Effects* page) and `utilities-system-monitor-symbolic` (the analysis
  category) are not part of Adwaita, so they were drawn from an unrelated theme
  or replaced by the missing-image glyph. Each effect now has its own icon.
- The effect chain column no longer grows with the length of an effect summary.
  The two columns are an `adw::OverlaySplitView` with a fixed sidebar width, so
  the editor keeps enough room for its controls: a parameter labelled *Release*
  no longer wraps in the middle of the word.
- A window narrower than 860 px collapses the chain into an overlay with a header
  toggle instead of squeezing both columns.
- A device node that leaves the graph is now removed from the discovered device
  list; the registry listener handles `global_remove`.
- The virtual sink no longer inherits a volume or a link target remembered by
  the session manager. Both internal streams opt out of the session manager's
  restore hooks, so the processing path always runs at unity gain. A stored
  volume for the sink name previously attenuated everything entering the chain:
  a −20 dB tone was measured at −55 dB at the input.
- EasyEffects preset conversion now honours the reference defaults. An omitted
  filter slope is order 1 (`x1`), an omitted de-esser mode is wideband (`Wide`),
  and an omitted delay time is 0 ms; these were previously replaced by the middle
  of the range here.
- EasyEffects conversion no longer drops content silently. The two ladder filter
  types are mapped to their closest shape instead of low pass, an unsupported
  compressor sidechain mode is reported, the equalizer folds *both* channel
  blocks into the shared curve and reports a right channel that differs, the
  crystalizer's per-band settings are reported instead of being filtered out of
  the warning list, and a delay whose left and right wet levels differ is
  reported because one mix cannot carry both.
- Preset conversion warnings now summarise a numbered family of settings (for
  example `band*`) instead of listing every band of a crystalizer preset.
- A reference preset's per-plugin `input-gain` and `output-gain` are no longer
  dropped without a word. They are transferred where an equivalent control
  exists (equalizer, limiter input, stereo tools input, bass enhancer and
  crystalizer output) and reported everywhere else.
- The equalizer conversion no longer turns a muted band into an audible one, and
  a soloed band excludes the others as it does in the reference. Bands without a
  usable frequency, per-band `mode`/`slope`/`width`, and a right channel that
  differs only in `mute` or `solo` are all reported.
- A reference document whose parameter block is keyed without the `#<instance>`
  suffix, which the reference still reads, is now found here too.
- A preset file whose name is not a valid preset name is no longer offered in
  the browser, and a malformed file no longer makes the browser and the loader
  disagree about which preset a name refers to: the malformed file is reported
  and the entry that is shown is the entry that loads.
- The preset browser and the loader agree on precedence. A file in a system
  preset directory shadows a compiled-in preset in both, so the entry that is
  shown is the entry that is loaded, and only files in the user directory are
  offered for deletion.
- EasyEffects preset conversion now reads the keys and labels the reference
  actually writes. Shelf filter types, the wideband de-esser mode, the delay
  times, the gate curve threshold and reduction, the auto gain silence threshold
  and the stereo tools side level were previously ignored and silently replaced
  by defaults; the mix level of a fully dry effect was inverted.
- EasyEffects conversion warnings and storage warnings now reach the interface
  instead of being discarded while loading or importing a preset.

### Changed

- The effect chain and the editor are laid out with `adw::OverlaySplitView`, and
  the pages are reachable with the `Ctrl`+`1`, `Ctrl`+`2` and `Ctrl`+`3`
  accelerators.
- The chain sidebar is quieter: each row carries the effect's own symbolic icon
  and an enable switch, the reorder and remove actions live behind a menu button
  revealed on hover, and one plain sentence reports the engine state. The earlier
  summary line and coloured status dot were removed because they duplicated the
  rows and did not say what they measured.
- The live PipeWire processing test opens its own virtual sink under a
  process-specific name, so the level it measures is the tone it played rather
  than whatever the session manager has routed to the shared sink.
- The reference conversion is split per effect family, and every reference
  setting that has no equivalent here is reported rather than dropped.
- The equalizer conversion folds the reference bands using the reference band
  quality as the shared bandwidth.

### Documentation

- `docs/architecture.md` describes the destination resolution, the icon resource,
  the split-view layout and how the interface was verified.
- `docs/reference-audit.md` records the completed audit of the reference
  checkout, including what was excluded and why, and how a session manager
  treats a virtual device.
- `docs/feature-matrix.md` documents the reference conversion per effect.
- `SECURITY.md` no longer describes `unsafe` blocks that do not exist in the
  codebase.

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
- Presets: one built in (Default), JSON on disk, with save, delete, import,
  export, value coercion, name validation, atomic writes and non-fatal
  reporting of unknown content. EasyEffects presets are recognised and
  converted on load, reporting everything that could not be mapped.
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

- The unit and integration suite, which needs neither a display nor an audio
  device; the current test count is listed in
  [docs/feature-matrix.md](docs/feature-matrix.md).
- Three live tests against a running PipeWire daemon: a tone is fed into the
  virtual sink with `pw-cat` and the level leaving the chain is measured.
  A representative run measured -20.00 dB at the input and -40.00 dB at the
  output for a -20 dB gain stage, and reported 4 096 underrun samples from the
  initial fill with no overruns and no trims.
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