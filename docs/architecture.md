# Architecture

This document describes the implementation that exists in this repository. It
is not a plan.

## 1. Crate graph

```text
pwe-core  ◄── pwe-dsp  ◄── pwe-engine ◄── pwe-ui ◄── pipewire-effects
    ▲                          ▲            ▲
    └────────── pwe-presets ───┴────────────┘
pwe-core  ◄── pwe-settings ────────────────►┘
```

| Crate | Owns | Does not know about |
|---|---|---|
| `pwe-core` | Audio buffers, parameter specifications, the effect contract, the chain, meters, realtime-safe helpers, the pipeline kinds | PipeWire, GTK, files |
| `pwe-dsp` | The twenty effects, the biquad designs and the effect registry | Threads, devices, presets |
| `pwe-presets` | The preset schema, name validation and the preset directory | Which effects exist |
| `pwe-settings` | The settings document, its atomic store and the autostart entry | The engine, the interface, the effects |
| `pwe-engine` | PipeWire streams, the virtual sink and source, the destination device, the realtime processor, counters | GTK, presets, settings |
| `pwe-ui` | GTK4/libadwaita widgets, the chain model, the engine bridge | DSP internals |
| `pipewire-effects` | Process entry point and the command line | Everything else |

Dependencies point one way. `pwe-dsp` depends only on `pwe-core`, which is what
makes every filter testable against its analytic frequency response without an
audio device. `pwe-presets` depends on `pwe-core` only to coerce stored values
into parameter types; it can be tested without PipeWire or GTK.

Nothing outside `pwe-dsp` names a concrete effect. The registry in
`crates/dsp/src/registry.rs` is the single list of effect identifiers, the
functions that build their parameter specifications, and the constructors that
turn a `ParamSet` into a `Box<dyn Effect>`. The interface renders whatever
specifications it finds.

## 2. The audio path

Two independent pipelines, both `pwe-engine` instances, each owning its own
connection, ring buffer, chain, counters and meters.

```text
  application ──► virtual sink (capture stream) ──► ring ──► processor ──► playback stream ──► device
                  media.class = Audio/Sink          SPSC     chain+meters    target.object
```

```text
  device ──► capture stream ──► ring ──► processor ──► virtual source (playback stream)
             Stream/Input/Audio   SPSC   chain+meters   media.class = Audio/Source
             target.object                                  applications record from it
```

The output pipeline is the one described in `README.md`: applications route into
`pipewire-effects-sink` and the processed audio is played on the chosen device.
The input pipeline reads the chosen capture device and publishes
`pipewire-effects-source`, so an application can record the processed
microphone. Each pipeline sets `node.group` and `node.link-group` to
`pipewire-effects`, and each only auto-connects the stream that is bound to a
real device: the virtual node is never auto-connected, so PipeWire does not try
to find a counterpart for it.

Both pipelines set `state.restore-props = false` and `state.restore-target =
false`. A session manager remembers stream volumes and link targets by name, and
the virtual nodes keep stable names, so a volume left behind by an earlier
session would silently scale everything entering the chain. These nodes are
internal, they must pass audio at unity gain, and the device each stream is
linked to is chosen by this application, not restored from metadata.

A ring buffer between the two streams of each pipeline absorbs scheduling
jitter. The alternative - one `pw_filter` node per effect, linked with
`link-factory`, exactly as the reference implementation does it - would remove
the buffer and the drift correction, but `pipewire-rs` 0.10 has no `pw_filter`
binding. Hand-writing that binding would concentrate unsafe code in the realtime
callback, which is the least testable place in the program. The two-stream design
uses only the safe API. The cost is documented in section 6.

### Why the DSP runs on the downstream side

The downstream stream of each pipeline is clocked by the hardware or by
whichever node drives the graph. The upstream stream is clocked by whatever is
feeding it. Running the chain on the downstream side gives the effects an even
cadence.

### Choosing the destination

Each pipeline binds the session manager's `default` metadata object and reads
its own key - `default.audio.sink` for the output pipeline and
`default.audio.source` for the input pipeline. It does this before it creates
its own node, so the value it sees is a real device rather than its own, and it
ignores any value that names one of its two internal nodes. A device chosen by
hand in the *Devices* page wins over the metadata and is persisted;
`default` in that control means "follow the system default" again. When the
metadata changes while the engine runs, the two streams are rebuilt for the new
device. If neither an explicit choice nor the metadata names a usable device, the
engine falls back to the highest `priority.session` among the discovered devices
of the right direction that are not itself and are not virtual.

`crates/engine/src/defaults.rs` owns the metadata watch, `crates/engine/src/target.rs`
owns the resolution policy, and both are covered without an audio server.

## 3. Realtime rules

The processor runs inside the PipeWire realtime callback. Three rules keep it
safe.

**No allocation, locking or freeing.** Parameters live in `AtomicU32` cells
(`f32` bits) shared with the interface. Structural changes travel as
`ChainCommand` values through a `crossbeam_queue::ArrayQueue` with capacity 64,
drained at the top of each block. Removed effects are pushed onto a second
queue (`ArrayQueue<Box<dyn Effect>>`, capacity 32) and dropped on the engine
thread, so the audio thread never calls the allocator. The chain is a
`Vec<Slot>` created with `Vec::with_capacity(16)` and never grows beyond it, so
`push`, `insert` and `remove` cannot reallocate.

**Bounded work.** A block drains at most 128 commands. Frames are processed in
sub-blocks of at most 8192 frames, which is also the capacity of the planar
processing buffer; a larger graph quantum is split rather than truncating.

**Changes are cheap.** Coefficient recomputation happens inside `process` when a
parameter signature changes. It is arithmetic only: no allocation, no I/O, no
logging, no system calls.

### Parameter transfer

`pwe_core::ParamSet` is an `Arc<ParamStore>`. `ParamStore` holds the immutable
specifications and a `Vec<AtomicU32>` of live values. Cloning a `ParamSet` shares
the same cells, which is what connects a slider in the interface to the DSP
instance running in the audio thread. `Knob` wraps a `ParamHandle` plus a
default, so a missing parameter degrades to the documented default instead of
panicking.

Values are validated on write: floats are clamped to their range and rejected if
non-finite, choices are clamped to the label count. The audio thread therefore
only ever reads values that are already legal.

### Chain structure

The interface and the audio thread apply the same commands in the same order,
which keeps their notion of the chain identical without a shared lock. The
engine keeps a `Vec<EffectSlot>` mirror for that purpose; the interface keeps the
authoritative list of slots, their parameter sets and their enable flags.

## 4. Threads

| Thread | Runs | Touches |
|---|---|---|
| GTK main | Widgets, actions, a 50 ms refresh tick | The active engine's snapshot, meters, statistics |
| `pipewire-effects-output` | PipeWire main loop for the output pipeline, `Loop::iterate` with a 20 ms timeout, command dispatch, effect disposal | Streams, device list, retired effects |
| `pipewire-effects-input` | The same for the input pipeline, started only when microphone processing is on | Streams, device list, retired effects |
| PipeWire realtime | Upstream submit and downstream processor, per pipeline | Ring buffers, chains, meters |

The interface never calls a PipeWire API. It sends `EngineCommand` values over a
`std::sync::mpsc` channel and reads an `Arc<Shared>` snapshot: atomics for
meters, counters and state, a mutex for the device list and diagnostics. The
mutex is never taken by the audio thread.

A failed stream does not leave the pipeline dead. The engine loop notices the
`Failed` state, waits an increasing backoff and rebuilds its two streams; the
Devices page also offers a manual *Reconnect*, which restarts the engine thread
and re-applies the chain from the parameters the interface still holds.

## 5. Drift and buffer policy

The ring holds 0.5 s of stereo audio at up to 192 kHz (192 000 samples),
allocated once. The target occupancy is the graph rate divided by 32 (31.25 ms
at 48 kHz).

| Situation | Who fixes it | How | Counter |
|---|---|---|---|
| Buffer empty | Consumer | Pad with silence | `underrun_samples` |
| Buffer full | Producer | Drop the newest samples | `overrun_samples` |
| Buffer above 1.5× target | Consumer | Discard the oldest samples | `trim_events` |

Silence is used rather than a repeated sample: a gap is less audible than the
click a repeated sample produces at a buffer boundary.

## 6. Known trade-offs

- **Two streams instead of filter nodes.** Adds a ring buffer and the corrections
  above. Replacing this with `pw_filter` would delete the ring buffer, the drift
  correction and the interleaving copies entirely; nothing outside `pwe-engine`
  would need to change.
- **Device changes rebuild both streams.** `pw_stream` properties cannot be
  changed after `connect`, so the ring is recreated and the virtual sink
  reappears. Audio stops for a fraction of a second.
- **Drift is trimmed, not resampled.** A resampler would preserve a sliver of
  audio instead of discarding it. On a healthy system the trim counter stays at
  zero.
- **No lookahead in the limiter.** Instantaneous attack guarantees the ceiling
  at the cost of some transient distortion.
- **The output device is chosen by `priority.session`.** This mirrors what the
  session manager uses to pick a default device, but it is a heuristic, not a
  read of the `default.audio.sink` metadata key.

## 7. The interface

`adw::ApplicationWindow` → `adw::ToolbarView` → `adw::ViewStack`, with an
`adw::ViewSwitcher` in the header as the top-level navigation. The three pages
also have `Ctrl`+`1`, `Ctrl`+`2` and `Ctrl`+`3` accelerators, registered as
window actions.

- **Effects**: an `adw::OverlaySplitView`. The start side is a fixed-width chain
  column (a pipeline switch, input meter, engine status, the effect rows, *Add
  effect*); the end side is the editor for the selected effect. Below 860 px the
  split view collapses and a header toggle reveals the chain over the editor. The
  switch chooses between the **Output** and **Input** chains; the sidebar and the
  editor always show the chain that is selected, and the meters follow it.
- **Presets**: the preset library, with load, save, delete, import and export.
- **Devices**: the engine status for the selected pipeline, its node name with a
  copy button, its destination chooser, a *Microphone* switch, a *Start on login*
  switch, statistics and diagnostics, and a *Reconnect* button that appears when
  the engine has failed.
- **Chain rows**: the effect's symbolic icon, its name and summary, and an enable
  switch; move up/down and remove are behind a menu button that appears on hover
  or secondary click. The labels are ellipsized so that the column width does not
  depend on the length of an effect summary. The column also carries the input
  meter and one line of engine status.
- **Editor**: a `adw::PreferencesPage` generated from the selected effect's
  parameter specifications. Booleans become `adw::SwitchRow`, choices become
  `adw::ComboRow`, floats become a labelled `gtk::Scale` that maps through the
  parameter's normalisation (so frequency sliders are logarithmic). Grouping
  comes from the `group` field of each specification.
- **Equalizer**: an analytic magnitude response drawn with cairo above the band
  sliders, recomputed from the live parameter values on every redraw.
- **Dialogs**: effect picker with search, and the name, confirm and file dialogs
  used by the preset library.
- **Meters**: `gtk::LevelBar` for peak with an RMS offset marker, plus a numeric
  readout. Both meters run through `LevelMeter`, so the two readouts are
  comparable when the chain is empty.
- **Banner**: engine errors and conversion warnings appear in an `adw::Banner`
  with a *Dismiss* button. The meter's hold and RMS markers and the equalizer's
  band markers are drawn from the widget's own foreground colour, so each stays
  visible in both appearances; a fixed colour made the hold and RMS markers
  invisible in one appearance and the band markers invisible in the light one.

The window polls the engine every 50 ms. Nothing blocks: the snapshot is a short
mutex acquisition on a plain data structure. The poll also compares a small
signature of the active chain - its pipeline, its length, whether it runs and its
highest slot - and rebuilds the rows when it changes, so a change made anywhere
else is picked up without a manual refresh.

### Internationalization

Every user-facing string is translated through gettext. `pwe_core::i18n` owns the
whole mechanism: it calls `setlocale`, binds the `pipewire-effects` domain to the
catalog directory (`/usr/share/locale`, or `PIPEWIRE_EFFECTS_LOCALEDIR`), and
exposes `t`, `tn` and `tr`. `tr` substitutes `{0}`, `{1}` placeholders in the
translated template, so a translator can reorder them.

The audio thread must not allocate, so `t` builds the lookup key in a fixed stack
buffer and returns the message identifier whenever `gettext` hands the buffer
back untranslated; only a real translation, which lives in the loaded catalog,
is borrowed. The catalog is loaded once, at startup, by `bind`.

Effect labels, parameter labels, parameter groups and choice labels stay plain
English where they are declared, in `pwe-dsp`, and are translated where they are
read, in `crates/ui`. An effect author therefore never calls gettext, and the
registry's uniqueness checks still compare identifiers rather than translations.

`t`, `tn`, `tr` and a no-op `mark` are the four extraction keywords. `mark`
returns its argument unchanged and exists only so `xgettext` can see a label
declared outside a `t()` call: the registry's effect names, categories and
summaries, the filter and choice tables, the equalizer band labels, and the
pipeline names. The remaining parameter labels and group names are literal
arguments to the parameter builders, so `scripts/update-po.sh` teaches
`xgettext` those builders (`float:2`, `boolean:2`, `choice:2`, `group:1`) instead
of repeating every label in a `t()` call. Without both, the template carried none
of that vocabulary and a translator had no message to translate.

The catalog lives in `po/`: `POTFILES.in` lists the sources, `LINGUAS` the
shipped languages, and `scripts/update-po.sh` regenerates the template. `make
install` compiles the `.po` files with `msgfmt`. No translation ships yet, so an
installed build shows English. `docs/feature-matrix.md` records the status.

### Icons

`data/io.github.D1ZZY4.PipewireEffects.gresource.xml` compiles the application
icon into the binary; `crates/ui/build.rs` runs `glib-compile-resources` and the
result is registered at startup, so the About dialog and the header bar find the
icon whether or not `data/icons` was installed. `crate::format` is the single
list of icon names, one per effect plus the chrome, and `format::missing_icons`
checks them against the live icon theme at startup and in a test. Every palette
name is part of the Adwaita theme: a name that only exists in an unrelated theme
would be drawn with a foreign glyph.

The application icon was redrawn as a hand-written vector - a gradient ring
around five pill-shaped level bars over soft background washes on a full-bleed
rounded tile - so it stays legible from 32 px up and follows the shell's corner
mask.

## 8. Presets

```json
{
  "schema_version": 1,
  "name": "Bass boost",
  "output": {
    "bypassed": false,
    "effects": [
      { "kind": "equalizer", "params": { "band1_gain": 7.0 } },
      { "kind": "limiter", "params": { "limit": -1.0 } }
    ]
  }
}
```

Decisions and reasons:

- **Unknown effects and parameters are reported, not fatal.** Loading a preset
  returns a list of warnings; the rest of the chain still loads.
- **Values are coerced.** A boolean written for a float parameter becomes `1.0`;
  text is matched against choice labels; numbers are clamped to their range.
- **Names are validated.** Path separators, `..`, leading dots, empty names and
  names longer than 96 bytes are rejected, so a preset name can never escape the
  preset directory.
- **Malformed files are skipped, not fatal.** The browser reports them and keeps
  the other presets.
- **Reference presets are converted, and what could not be converted is
  reported.** A warning list travels back with the loaded preset: the banner
  shows a summary, and the preset list shows every warning in full. The mapping
  per effect is in `docs/feature-matrix.md`.
- **A user preset shadows a built-in and stays deletable.** The lookup order is
  the user directory, then the system directories, then the compiled-in set, and
  the browser uses the same order, so the entry that is shown is the entry that
  is loaded. Only a file in the user directory can be deleted.
- **Writes are atomic.** A temporary file is renamed into place.
- **Only non-default parameters are written**, which keeps saved presets short
  and reviewable.

Built-in presets are compiled in, so a fresh installation has a usable set
without a system preset directory.

## 9. Settings, the command line and packaging

`pwe-settings` owns one document, `$XDG_CONFIG_HOME/pipewire-effects/settings.json`:

```json
{
  "output": { "device": "default", "bypassed": false },
  "input":  { "device": "default", "bypassed": false },
  "preset": "Default",
  "process_input": false,
  "autostart": false
}
```

It is written atomically through a temporary file, and a damaged document falls
back to the defaults with a message instead of stopping the application. The
document is what makes the device choice and the loaded preset survive a restart;
previously both were lost.

`pwe-settings` also owns the autostart entry. *Start on login* writes
`$XDG_CONFIG_HOME/autostart/io.github.D1ZZY4.PipewireEffects.desktop` with
`Exec=pipewire-effects --no-window`, so the processing starts at sign-in without
a window; launching the application again activates the running instance and
opens its interface.

The binary parses its own flags before GTK sees them: `--help`, `--version`,
`--preset NAME`, `--device NAME`, `--no-window`, `--list-presets` and
`--list-devices`. `crates/app/src/cli.rs` owns the parsing and is covered by
tests; the parsing is deliberately hand-written rather than pulling in an
argument-parsing dependency for seven flags.

`make install` installs the binary, the desktop entry, both icon sets and the
built-in preset below `$(PREFIX)`. `packaging/` carries a `PKGBUILD` and a
Flatpak manifest. The `make install` tree was inspected; no package was built or
installed into the system here.

## 10. Error handling

- `CoreError`, `PresetError` and `EngineError` are explicit enums with `Display`
  and `Error` implementations. No error is discarded silently.
- PipeWire stream failures set the shared state to `Failed` and publish the
  message; the interface shows it in a banner.
- A missing output device does not stop the chain: it is reported as a warning
  and the destination is marked unavailable in the settings page.
- Preset problems carry the offending name or file path.

## 11. Verification

- `cargo test --workspace`: 317 tests, no audio device needed.
  Two of them need a display and skip themselves without one: the bundled icon
  lookup and the icon-theme check.
- The registry checks itself: effect identifiers and labels are unique, every
  effect publishes parameters and builds, and every parameter identifier is
  unique within its effect.
- Filter designs are checked against their analytic magnitude response, `Q` and
  gain sweeps, and a pole-stability sweep.
- Effects are checked against measured levels: 20 dB of gain, −6 dB of gain,
  limiter ceilings with 20 dB of drive, compressor ratios, gate range and hold,
  delay timing, reverb tail decay, de-esser selectivity, and numerical bounds
  under abusive settings.
- The De-esser is checked for a live detector change: moving *Sibilance* while it
  runs must release a tone the detector was holding, which fails if the band-pass
  is only computed once.
- The reverberation pre-delay is checked for channel isolation: an impulse in the
  right channel must not appear at the left output, which fails if the two
  channels share one pre-delay line.
- The registry carries each descriptor's parameter specifications, so a
  descriptor and its parameter table cannot drift apart.
- The ring buffer, command queue and trimming policy are exercised with a real
  `rtrb` producer and consumer.
- The two meters are checked against each other: an empty chain must report the
  same input and output level for the same block.
- Presets are round-tripped through JSON, hostile names are rejected, malformed
  files are skipped.
- Reference presets are converted against the keys and labels the reference
  actually writes, and the warnings that report what was dropped are covered,
  including that they reach the caller through the store.
- The metadata document that carries the default sink is parsed against the
  spellings the session manager writes.
- `crates/engine/tests/live_pipewire.rs` runs the real engine against the running
  daemon. The processing test opens its own virtual sink under a
  process-specific name so that unrelated applications cannot be routed into it,
  feeds a −20 dB tone with `pw-cat` into that sink and asserts both the level the
  tone reaches the chain at and the level reduction of a −20 dB gain stage. The
  input test starts the microphone pipeline and records from
  `pipewire-effects-source` with `pw-cat --record`, then asserts that the
  virtual source exists and that the recording drove the input chain. Four tests,
  `#[ignore]`d so they do not run by default.
- The command line is parsed without a dependency and covered by tests: the flags
  in both spellings, the rejection of an unknown option, and that the help text
  lists every option.
- The settings document round-trips through its file, a missing file loads
  without a warning, and a corrupt file falls back to the defaults with one.
- `PIPEWIRE_EFFECTS_SMOKE_TEST=1` builds the real window, adds an effect, loads
  the *Default* preset and exits with a status. It requires a display and a
  running PipeWire.
- `PIPEWIRE_EFFECTS_UI_STATE=<state>` drives the real window into one state after
  it is mapped, so a screenshot can be taken of a specific screen. The states are
  `chain`, `editor`, `equalizer`, `presets`, `devices`, `input`, `picker`,
  `picker-tone`, `about`, `save`, `banner`, `bypass`, `narrow` and
  `effect:<kind>`, which loads the built-in preset, adds the named effect if it
  is absent and selects it. This is the harness the interface review in section 13
  used; it is not part of the product.
- `scripts/capture-ui.sh` runs the harness over an isolated headless GNOME
  session and writes one PNG per state; `scripts/README.md` documents it. It
  starts its own D-Bus session and its own `XDG_CONFIG_HOME`, so it never touches
  the settings, presets or autostart entry of the person running it, and points
  only `PIPEWIRE_RUNTIME_DIR` at the live daemon.
- `scripts/update-po.sh` regenerates `po/pipewire-effects.pot` and merges the
  translation catalogs. See the internationalization subsection of section 7.
- `PIPEWIRE_EFFECTS_UI_CAPTURE=<path>` renders the active window through its own
  render tree and writes the result to that PNG. It exists because GNOME refuses
  `org.gnome.Shell.Screenshot` to every client except the shell itself, so a
  compositor screenshot is not available to a test on a Wayland session.
- A second, independent capture channel exists for the X11 backend: the release
  binary runs under `Xvfb` and an external client takes the whole root window with
  ImageMagick's `import -window root`. That is a screen capture taken by another
  process rather than the window rendering itself, and it was used to check the
  main window and the Devices page in both appearances.
- The accessibility tree is checked over the real AT-SPI bus. The application is
  started with `PIPEWIRE_EFFECTS_UI_STATE` set, its unique name is found on the
  accessibility bus, and `org.a11y.atspi.Accessible.GetRoleName` plus the `Name`
  property are read for every object down the tree. On the development machine
  this exposed the window (`PipeWire Effects`), the navigation (`Effects`,
  `Presets`, `Devices` tabs), the header controls (`Bypass the effect chain`,
  `Main menu`, `Minimize`, `Maximize`, `Close`), every `switch` by name, the
  destination `combo box`, the `list item` rows, and the parameter `slider`s named
  after their labels (`Sibilance`, `Threshold`, `Ratio`, `Attack`, `Release`) with
  their value text (`5600 Hz`, `-15.0 dB`, `1.35:1`, `5.0 ms`, `100.0 ms`). Orca
  itself is not installed, so the screen reader was not driven end to end; the
  interface it consumes was inspected directly.
- `make install DESTDIR=<staging> PREFIX=/usr` installs the binary, the desktop
  entry, both icon sets and the built-in preset into a staging tree, and
  `make uninstall` removes exactly what it installed; the staging tree was
  produced, listed, checked with `desktop-file-validate`, and removed again. The
  `PKGBUILD` was not built here because its source is the GitHub release tarball
  and this environment has no network, and `flatpak-builder` is not installed.

## 12. Performance

- The audio path performs two interleaving copies per block (SPA bytes → planar
  `f32` → SPA bytes) and no allocation.
- Effects iterate per channel over contiguous planar slices; no per-sample
  virtual dispatch beyond the effect boundary.
- Coefficient recomputation and knob reads happen once per block, not per sample.
- The interface refreshes at 20 Hz and rebuilds only the widgets that changed
  (chain list or editor), never on a timer.
- Measured during the live test on the development machine: a −20 dB tone fed
  through a −20 dB gain stage was measured at −20.00 dB at the input and
  −40.00 dB at the output, with 16 384 frames captured and processed, 4 096
  underrun samples from the initial fill, and no overruns or trims.

### CPU baseline

`cargo run --release -p pwe-dsp --example benchmark` times each effect and the
full twenty-effect chain on the machine it runs on: a 48 kHz stereo stream, a
per-block wall clock over 2 000 blocks, and the tone-generation cost subtracted
so the numbers are the effects. On the development machine (Intel iGPU, four
cores, `rustc 1.99.1`, release profile with thin LTO) a representative run gave:

| Workload | 1024 frames per block | 128 frames per block |
|---|---|---|
| All twenty effects | 646 µs/block · 33.0× realtime · 3.03 % of one core | 81 µs/block · 32.9× realtime · 3.04 % of one core |
| Reverberation (heaviest) | 93 µs/block | 11.5 µs/block |
| Mid/side equalizer | 85 µs/block | 10.6 µs/block |
| Equalizer | 78 µs/block | 9.8 µs/block |
| Gain (lightest) | 8 µs/block | 0.9 µs/block |

The whole chain uses about 3 % of one core and leaves roughly thirty times the
deadline free at both quantum sizes, which is the headroom the realtime callback
needs for scheduling jitter. Repeated runs on the loaded development machine
measured between 25× and 33× realtime, or about 3 % to 4 % of one core, so the
figures are a range rather than a single number. These are wall-clock figures from
one machine, not a portable claim; re-run the example to reproduce them.

### GPU acceleration

No GPU backend is implemented, and the numbers above are the reason. The entire
chain costs about 3 % of a single core, so off-loading any of it would add a
transfer, a synchronisation point and a dispatch to a callback whose budget is
21 ms (2.7 ms at 128 frames), while the effects are recursive per-sample filters
with a serial dependency that does not parallelise. The only workload that could
benefit is FFT convolution, which is out of scope because the project carries no
FFT dependency. The machine does expose an Intel iGPU through a Mesa Vulkan ICD
(`intel_icd.json`, `libvulkan.so.1.4.363`), so a Vulkan compute backend is
technically available; it is deliberately not added, and the workspace carries no
GPU dependency.

## 13. Interface review

The first pass reviewed the interface by running the release binary under a
separate X server (`Xvfb`, the X11 backend) and capturing the window with
ImageMagick's `import` for each harness state listed in section 11, in both the
dark and the light appearance. A later pass repeated the review on Wayland,
which is the platform this application targets; see the end of this section.
What the first pass changed:

- The bundled application icon: the About dialog and the header bar showed the
  missing-image glyph, and the icon-theme resource path had to be the theme
  directory rather than its parent.
- The destination chooser: the automatic entry was too long for the row and the
  row did not say which device had been resolved.
- The chain column: the longest summary stretched the column and pushed the
  editor into wrapping *Release* in the middle of the word. The split view and
  the ellipsized labels fixed that.
- The tone and analysis effect icons: both names were absent from Adwaita.
- A window narrower than 860 px: the sidebar now collapses.
- The chain sidebar: a summary line ("N effects · M active") and a coloured
  status dot were removed. The count is evident from the rows themselves, and the
  dot did not say whether it meant engine state or bypass; the engine status is
  now one plain sentence ("48000 Hz · 2 channels", "Bypassed"), and the enable
  switches are the only indicator on a row.
- The effect identity: an earlier pass drew each effect as a tinted, boxed
  category chip. That read as decoration rather than information, so the chip was
  replaced with the effect's plain symbolic icon, drawn in the theme's own
  foreground colour with no background, matching the rest of the desktop.
- The row menu: move and remove were always present and competed with the switch;
  the menu button is now revealed on pointer hover and on a secondary click,
  so the switch is the only control at rest.

The narrow window and the light appearance were captured after each fix. The
live meters were captured with the session's real audio routed through the
virtual sink, which is what exposed the two meters disagreeing.

A later pass added seven effects and re-ran the review for the states they touch
(`PIPEWIRE_EFFECTS_UI_STATE=effect:<kind>`, which loads the built-in preset,
adds the effect if it is absent and selects it). What it changed:

- The two meters were on opposite sides of the split view — the input below the
  chain header, the output above the editor — so a chain of four or more rows
  pushed them apart and they could not be read together. Both now sit in one
  horizontal row under the header, above whichever page is showing, and both
  expand to fill it.
- `gtk::LevelBar` drew a faint level trace for a silent meter. The history trace
  is skipped while every stored value is at the floor.
- The parameter sliders carried no accessible name or value, so a screen reader
  announced only "slider". Each slider now publishes
  `gtk::accessible::Property::Label` and `ValueText`, and the value text is
  refreshed on every change, with the unit and the current value.
- The window did not set an icon name at all, and the header bar fell back to
  the missing-image glyph on a machine without `data/icons` installed; the
  bundled icon is now registered under the theme directory itself.
- New effects were checked for a distinct icon, a distinct label, a parameter
  list that builds and a preset mapping, and the registry now asserts all four.

No PipeWire daemon was restarted, no device configuration was written, and no
system setting was changed for the review. The light appearance was captured
with a private `GSETTINGS_BACKEND=memory` and `ADW_DISABLE_PORTAL=1`, so the
development machine's own colour scheme was not touched.

The review was repeated on Wayland, which is the platform this application
targets. `gnome-shell --headless --wayland --virtual-monitor` was started in a
private D-Bus session, the release binary ran against it with
`GDK_BACKEND=wayland`, and each state was captured with
`PIPEWIRE_EFFECTS_UI_CAPTURE`. The shell's own screenshot interface answers
`AccessDenied` to a client, so the window renders itself; the file is the
window's own render tree at its allocated size, not a compositor capture of the
screen. `ADW_DEBUG_COLOR_SCHEME` selected the appearance.

What that pass changed:

- The `narrow` harness state set the window size after the window had been
  mapped, which GTK4 ignores, so no narrow capture had ever exercised the
  collapsed layout. The size is now set before the window is mapped and the
  state is captured at 720x620, where the sidebar is collapsed and the header
  toggle appears.
- The About dialog is a separate toplevel, so capturing the main window found no
  render node. The capture now follows the application's active window.
- The application icon was redrawn against a reference mark: a thicker gradient
  ring, five pill-shaped level bars and two soft background washes on a
  full-bleed rounded tile. It was checked rendered at 32, 64, 128 and 512 px and
  in the About dialog in both appearances.

Every destination above was captured in the light appearance, and the chain, the
equalizer editor, the effect picker, the About dialog and the cross-talk
canceller editor were also captured in the dark appearance. The seven effects
added by the previous pass were each captured in their own editor state. No
clipping, overlap, truncation or missing glyph was found in those captures.

A further pass covered the two pipelines. It captured the Effects page with the
pipeline switch on *Output* and on *Input*, the Devices page, the presets page
and the About dialog, in the light appearance and on the headless Wayland
session described above. The input capture showed the real microphone running
through the input chain, both meters reading −36.8 dB, which is what confirmed
that the second pipeline is fed by the capture stream and not merely drawn. The
Devices capture, taken from a settings file that had microphone processing
enabled, showed the switch already on, which is what confirmed that the settings
document is read and applied at startup. The interface at 720x620 was captured
again after the sidebar gained the pipeline switch and still collapses without
clipping.

A further pass reviewed the changes made in the increment that fixed the
De-esser's live parameters, the reverb pre-delay and the translation template, on
the same headless Wayland session, in both appearances. The captures found two
defects that no test could see, and both are fixed: the equalizer's band markers
were drawn in a fixed near-white that is invisible on the light appearance (they
are drawn from the widget's own foreground colour now and are visible in both),
and the error banner could not be dismissed by hand because its button was never
shown even though a click handler was wired to it (the banner carries a
*Dismiss* button now). The input pipeline was captured again and read −36.8 dB on
both meters, which is the real microphone feeding the chain. The De-esser,
compressor, expander, reverberation and equalizer editors were captured after the
change: each renders its groups and its controls, and the De-esser's *Sibilance*,
*Attack* and *Release* rows are the ones the live-parameter fix made effective.
No clipping, overlap, truncation or missing glyph was found.

A final pass checked which effect a chain row is showing. It found that the rows
never marked the effect the editor belonged to. Rebuilding the chain list
recreated every row and dropped the `GtkListBox` selection, so the highlight was
gone as soon as the 50 ms poll noticed a change in the chain, and also after a
preset load, a move or an enable toggle; the editor and the chain then disagreed
about which effect was selected. The selection is restored from the chain model
whenever the rows are rebuilt or the editor is refreshed, and the highlighted row
was captured in both appearances: the row fills with the theme's selection colour
instead of staying flush with its neighbours. The pass re-ran all thirteen harness
states in `prefer-light` and `prefer-dark` and re-checked the real microphone
pipeline, which read −36.8 dB at the input meter and −36.9 dB at the output meter,
so the input chain is fed by the capture stream rather than drawn.

The gettext pipeline was exercised end to end for the first time. A small
Indonesian catalog was compiled with `msgfmt` and bound with
`PIPEWIRE_EFFECTS_LOCALEDIR`; the navigation then read *Efek*, *Preset* and
*Perangkat*, the chain header read *Rantai*, and the effect names read *Ekualiser*
and *Pembatas*, while the strings the test catalog did not carry stayed English.
The mechanism therefore translates and falls back as designed; no translation is
shipped, so an installed build still shows English.
