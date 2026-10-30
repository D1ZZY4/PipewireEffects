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
| `pwe-dsp` | The FFT, the Linkwitz-Riley crossover, the biquad design, the twenty-three effects, the R128 loudness measurement and the effect registry | Threads, devices, presets |
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

`adw::ApplicationWindow` → `adw::OverlaySplitView`. The end side is an
`adw::ToolbarView` holding the header, the banner and an `adw::ViewStack`; the
start side is a `gtk::Stack` that shows one sidebar per pipeline. The `ViewStack`
has two titled children, **Output** and **Input**, and its `adw::ViewSwitcher` is
the header's title widget, so the header carries no window-title text. The two
pipelines have `Ctrl`+`1` and `Ctrl`+`2` accelerators, registered as window
actions. Below 860 px the split view collapses: the editor fills the window, the
switcher drops to its icon-only policy, and the sidebar opens as an overlay
drawer. The window's minimum size is 360x480, so the same layout serves a
phone-width screen.

The header's start holds a sidebar toggle (`sidebar-show-symbolic`) that shows or
hides the sidebar at any width; the main menu is at the end and carries **Devices**
(`Ctrl`+`4`), **Preferences** (`Ctrl`+`,`), **About PipeWire Effects** and
**Quit**. The two device destinations open `adw::PreferencesDialog`s rather than
occupying a page.

- **Sidebar**: one per pipeline, so switching the switcher never rebuilds the
  other pipeline's widgets. It carries the theme's sidebar background, with the
  split view's 1 px separator switched off in the application stylesheet, so the
  panel is distinct without a drawn line. Its first line carries the chain on/off
  **Effects** toggle, *Add effect* and, while the split view is collapsed, a
  *back* button that closes the drawer; below it is the effect list, and then a
  **Presets** section with the preset list and the save, import, export and delete
  buttons. A pipeline with no preset of its own and no skipped file shows a
  one-line empty state instead, and skipped files are listed in full in a
  *Skipped files* row.
- **Effects toggle**: the chain on/off, a labelled toggle button at the top of the
  sidebar. It is accented while that pipeline's chain processes audio and plain
  while it is bypassed, which is the state a user reads at a glance. It replaced
  an icon-only control whose meaning was the opposite (an unchecked *Bypass*
  switch), so a running chain looked switched off. Each pipeline has its own
  toggle, and it drives that pipeline's bypass.
- **Editor and meters**: the end side carries the chain's input and output meters
  above the editor for the selected effect.
- **Devices** (menu): the format and quantum of the active pipeline, its node
  name with a copy button, its routing hint, its destination chooser, a
  *Microphone* switch and a *Reconnect* button that appears when the engine has
  failed.
- **Preferences** (menu): *Start on login* and a *Developer mode* switch. The
  engine counters and the diagnostic messages are shown in a *Diagnostics* group
  only while developer mode is on, and the choice is persisted.
- **Chain rows**: one flat line per effect, in processing order - the effect's
  symbolic icon, its name, an on/off `gtk::Switch` and the row menu. The icon is
  drawn in the theme's foreground colour, not the accent, and dims while the
  effect is disabled, as does the name; the switch carries the accent and is the
  enable control. Clicking the row selects it for the editor; the summary is the
  row's tooltip, and move up/down and remove are behind a menu button that appears
  on hover or secondary click. The rows
  carry no category headers and no position numbers: the visible order is the
  processing order, so *Move up* and *Move down* always move an effect past the
  neighbour that is drawn above or below it. The effect picker applies the same
  vocabulary but groups its rows by category, which is where that grouping
  belongs.
- **Editor**: a `adw::PreferencesPage` generated from the selected effect's
  parameter specifications. Booleans become `adw::SwitchRow`, choices become
  `adw::ComboRow`, floats become a labelled `gtk::Scale` that maps through the
  parameter's normalisation (so frequency sliders are logarithmic). Grouping
  comes from the `group` field of each specification. A float row keeps its label
  on one line and gives the scale a 120 px floor, so a 360 px window ellipsizes a
  long label instead of breaking it across lines.
- **Equalizer**: an analytic magnitude response drawn with cairo above the band
  sliders, recomputed from the live parameter values on every redraw.
- **Dialogs**: effect picker with search, and the name, confirm and file dialogs
  used by the preset library.
- **Meters**: `gtk::LevelBar` for peak with an RMS offset marker, plus a numeric
  readout. Both meters run through `LevelMeter`, so the two readouts of one
  pipeline are comparable when its chain is empty.
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
  "autostart": false,
  "developer_mode": false
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
argument-parsing dependency for seven flags. The application is started with
`Application::run_with_args` and only the program name, so GLib's own argument
parser never sees the options; `Application::run` hands it the whole process
command line, and it rejects any option it does not know.

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

- `cargo test --workspace`: 343 tests, no audio device needed.
  Two of them need a display and skip themselves without one: the bundled icon
  lookup and the icon-theme check.
- The R128 measurement is checked against the standard's own calibration: a
  full-scale sine measures about 0 LUFS, a −23 dBFS sine measures −23 LUFS within
  0.4 LU, a 10 dB step moves the measurement by 10 LU, and silence is not
  measured at all.
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
  `chain`, `editor`, `equalizer`, `presets`, `devices`, `preferences`,
  `developer`, `input`, `picker`, `picker-tone`, `about`, `save`, `banner`,
  `bypass`, `narrow`, `mobile`, `mobile-drawer`, `sidebar-hidden` and
  `effect:<kind>`, which loads
  the built-in preset, adds
  the named effect if it is absent and selects it. `devices` and `preferences`
  activate the `win.devices` and `win.preferences` actions, so the menu wiring is
  exercised too. This is the harness the interface review in section 13
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
  property are read for every object down the tree. That run predates the sidebar
  redesign in section 7; on the development machine it exposed the window
  (`PipeWire Effects`), the navigation of the time (`Effects`, `Presets`,
  `Devices` tabs), the header controls (`Bypass the effect chain`, `Main menu`,
  `Minimize`, `Maximize`, `Close`), every `switch` by name, the
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
full twenty-three-effect chain on the machine it runs on: a 48 kHz stereo stream, a
per-block wall clock over 2 000 blocks, and the tone-generation cost subtracted
so the numbers are the effects. On the development machine (Intel Core i7-7500U,
two cores and four threads, `rustc 1.99.1`, release profile with thin LTO) a
representative run gave:

| Workload | 1024 frames per block | 128 frames per block |
|---|---|---|
| All twenty-three effects | 1743 µs/block · 12.2× realtime · 8.17 % of one core | 272 µs/block · 9.8× realtime · 10.2 % of one core |
| Voice suppressor (heaviest) | 418 µs/block | 104 µs/block |
| Multiband compressor | 285 µs/block | 37 µs/block |
| Multiband gate | 266 µs/block | 35 µs/block |
| Reverberation | 101 µs/block | 12.3 µs/block |
| Equalizer | 81 µs/block | 9.5 µs/block |
| Gain (lightest) | 1.9 µs/block | 0.04 µs/block |

The whole chain uses between about 8 % and 10 % of one core and leaves roughly
ten to twelve times the deadline free at both quantum sizes, which is the
headroom the realtime callback needs for scheduling jitter. The three spectral
effects added last are the expensive ones: the voice suppressor alone is a
quarter of the chain at 1024 frames because it runs a 1024-point STFT on every
block. Repeated runs vary with machine load: two back-to-back runs of the same
binary measured 12.5x and 12.2x realtime at 1024 frames and 10.0x and 9.8x at 128
frames, so the figures are a range rather than a single number. At 128 frames the
cheapest effects measure at or below the timer resolution, so their small-block
figures are a floor, not a precise cost. These are wall-clock figures from one
machine, not a portable claim; re-run the example to reproduce them.

### GPU acceleration

No GPU backend is implemented, and the numbers above are the reason. The entire
chain costs under a tenth of a single core, so off-loading any of it would add a
transfer, a synchronisation point and a dispatch to a callback whose budget is
21 ms (2.7 ms at 128 frames), while the effects are recursive per-sample filters
with a serial dependency that does not parallelise. The only workload that could
benefit is FFT convolution, which is a different effect from the ones here (the
convolver is not implemented), so a GPU backend would accelerate nothing that
ships. The machine does expose an Intel iGPU through a Mesa Vulkan ICD
(`intel_icd.json`, `libvulkan.so.1.4.363`), so a Vulkan compute backend is
technically available; it is deliberately not added, and the workspace carries no
GPU dependency. The one FFT in the project is a hand-written radix-2 transform,
not an external library.

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

A later pass re-ran the harness on the current build on the headless Wayland
session: the fourteen light states and eleven dark states were captured and
inspected again, together with the twenty-three-effect editors where they differ. The
light pass covered `chain`, `editor`, `equalizer`, `presets`, `devices`,
`picker`, `picker-tone`, `about`, `banner`, `bypass`, `narrow`, the crusher and
reverberation editors, and the input pipeline; the dark pass repeated the
standard set and the crusher editor. The input pipeline read −36.8 dB on both
meters again, which is the real capture device feeding the second chain. No
clipping, overlap, truncation, missing glyph or theme-contrast problem was found.
The pass confirmed the *Dismiss* button on the error banner and the equalizer
band markers in both appearances. It found no new interface defect, but it
prompted a check of the command line, which found the `Application::run` defect
recorded in section 9: `--no-window`, `--preset` and `--device` were rejected by
GLib before the application saw them. The application is run with
`Application::run_with_args` now, and `--no-window`, `--preset Default`,
`--preset=Default`, `--device does-not-exist`, `--list-presets` and
`--list-devices` were each run against the rebuilt binary under a private bus and
behaved as documented.

The accessibility tree was probed again over a private AT-SPI bus
(`at-spi2-registryd` on a private `AT_SPI_BUS_ADDRESS`). It exposed the desktop,
the `pipewire-effects` application, the `PipeWire Effects` frame, the container
hierarchy and individual labels such as `Input`, `-inf dB` and `Engine` (twenty
to twenty-two objects per state), which confirms the tree is populated. This pass
did not reproduce the control-level walk recorded above: its traversal did not
yet reach the switches, combo boxes and sliders. That is a limitation of this
pass, not a regression; the earlier walk, with the slider names and value text,
remains the record for the interactive controls.

`scripts/capture-ui.sh` was re-run unchanged; `scripts/update-po.sh` regenerates
`po/pipewire-effects.pot` (372 messages), and
`make install DESTDIR=<staging> PREFIX=/usr` installed the
binary, the desktop entry, both icon sets and the built-in preset into a staging
tree and `make uninstall` removed exactly what it installed.

The most recent pass reviewed the navigation change and the R128 auto gain on the
same headless Wayland session, in the light appearance and again in the dark one.
It found and fixed two defects: the destination chooser's value was ellipsized in
the narrower Devices dialog, so the row was shortened to *Output device* /
*Input device*; and the first version of the *Developer mode* switch hid the
group it lived in, which would have made it impossible to switch back off. The
final captures show the *Output* and *Input* switcher pages with the preset list
in the sidebar, the Output chain
with the built-in preset, the Input chain driven by the real capture device
(−36.8 dB on both of its meters), the Devices and Preferences dialogs, and the
Preferences dialog with developer mode on and the engine counters visible. The
harness now reaches the two dialogs by activating the same `win.devices` and
`win.preferences` actions the menu uses, and the run reports no
"action could not be activated" line, which is how the menu wiring is checked
without a compositor screenshot of the popover. The settings file written by the
run carries `"developer_mode": true`, which is how the persistence of the new
switch was checked.

The R128 measurement is not verified by the interface pass. It is verified by the
unit tests listed in section 11, against the standard's own calibration values,
and it runs in the audio path through `AutoGain`.

The current pass reworked the shell and re-ran the harness in both appearances.
It moved the chain on/off **Effects** toggle from the header into the per-pipeline
sidebar, gave each effect row an on/off switch whose accent replaces the old
accent on the effect icon, dropped the *Output chain* / *Input chain* heading, made the switcher the
header's title widget so no window-title text is drawn, flattened the effect rows
into processing order with no category headers and no position numbers, and made
the layout adaptive: the split view collapses below 860 px, the switcher drops to
icons, a back button closes the drawer, and the window's minimum size is 360x480.
The sidebar keeps the theme's tinted background; the split view's own 1 px
separator is switched off in the application stylesheet. Pixel sampling across the
sidebar/content boundary at y=400 reads `rgb(235,235,237)` inside the sidebar and
`rgb(250,250,251)` in the content with no darker pixel between them, so the panel
is distinct without a drawn line. The run wrote 16 light captures and 15 dark
captures (`chain`, `editor`, `equalizer`, `presets`, `devices`, `preferences`,
`developer`, `picker`, `picker-tone`, `about`, `banner`, `bypass`, `narrow`,
`mobile`, `mobile-drawer` and `input`) into `target/ui-captures-light` and
`target/ui-captures-dark`. The same run's live PipeWire tests measured the input
chain at −36.85 dB in and −42.85 dB out (a −6 dB gain stage) over 12 288 processed
frames, and a −20 dB tone reduced by 20.00 dB over 16 384 captured frames with no
underruns, overruns or trims.

A later pass exercised the preset library by hand on the X11 backend under
`Xvfb`, where the sidebar buttons can be clicked with `XTest`; the frames were
taken from the X server with ImageMagick's `import -window root`, so they are a
capture by another process rather than the window rendering itself. It found that
loading, saving or deleting a preset aborted the process: each handler matched on
the result of a `State` mutation while the `RefCell` borrow was still alive, and
the matched arm re-entered the interface, which borrows the same cell; the release
profile is built with `panic = "abort"`, so the panic ended the process. The
result is bound to a local before it is matched now. The same clicks then loaded
the built-in preset (eight effects appeared and the live meters read −1.4 dB in
and −3.9 dB out), saved it under a new name (the file was written and the row
became a user preset) and deleted it (the file was removed and the row returned
to the built-in), with the process alive at every step. The shell helpers
(`install_breakpoint`, `sidebar_toggle`) were moved from `pages.rs` into
`chrome.rs` in the same pass, which brought every interface source file back
under the 350-line limit.
