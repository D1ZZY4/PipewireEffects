# Architecture

This document describes the implementation that exists in this repository. It
is not a plan.

## 1. Crate graph

```text
pwe-core  ◄── pwe-dsp  ◄── pwe-engine ◄── pwe-ui ◄── pipewire-effects
    ▲                          ▲            ▲
    └────────── pwe-presets ───┴────────────┘
pwe-core  ◄── pwe-services ────────────────►┘
```

| Crate | Owns | Does not know about |
|---|---|---|
| `pwe-core` | Audio buffers, parameter specifications, the effect contract, the chain, meters, realtime-safe helpers, the pipeline kinds | PipeWire, GTK, files |
| `pwe-dsp` | The FFT, the Linkwitz-Riley crossover, the biquad design, the thirty effects, the spectrum analyser, the R128 loudness measurement and the effect registry | Threads, devices, presets |
| `pwe-presets` | The preset schema, name validation and the preset directory | Which effects exist (in production code; its tests take `pwe-dsp` as a dev-dependency so the built-in presets can be checked against the registry) |
| `pwe-services` | The settings document, its atomic store and the autostart entry | The engine, the interface, the effects |
| `pwe-engine` | PipeWire streams, the virtual sink and source, the destination device, the realtime processor, the spectrum snapshot and the compute backends (`cpu`, `gpu`), counters | GTK, presets, services |
| `pwe-ui` | GTK4/libadwaita widgets, the chain model, the engine bridge | DSP internals |
| `pipewire-effects` | Process entry point and the command line | Everything else |

Dependencies point one way. `pwe-dsp` depends only on `pwe-core` and on
`nnnoiseless`, the Rust port of RNNoise that the noise reduction stage runs;
`nnnoiseless` is the only DSP dependency and it is confined to that one effect.
`pwe-dsp` has no thread, device or file knowledge, which is what makes every
filter testable against its analytic frequency response without an audio device.
`pwe-presets` depends on `pwe-core` only to coerce stored values into parameter
types; it can be tested without PipeWire or GTK.

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
                                                             +spectrum
```

```text
  device ──► capture stream ──► ring ──► processor ──► virtual source (playback stream)
             Stream/Input/Audio   SPSC   chain+meters   media.class = Audio/Source
             target.object                +spectrum       applications record from it
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

**The analyser is bounded.** The spectrum runs an 8192-point FFT of the mid
signal every third block, over history the analyser owns, and publishes 48 band
levels through atomics. Its buffers, window and twiddle tables are allocated when
the processor is built, not in the callback. It costs 117 μs per 1024-frame
block on the development machine, which is 0.55 % of one core (section 12), and
it is the largest single cost outside the effect chain.

**The GPU stage is bounded.** When `GPU` or `Both` is selected, the elementwise
gain stage runs on Vulkan. Its buffers, pipeline and warm-up are prepared
outside the callback; a block waits for the fence for at most half of that
block's duration, and a miss is applied on the CPU instead. See section 12.

**Latency is reported, not compensated.** An effect that delays its output
declares it through `Effect::latency_frames`, the chain sums the enabled slots,
and the processor publishes the total for the Diagnostics dialog. The limiter's
lookahead and the pitch shifter's analysis window are the stages that declare one
today. The engine does not ask PipeWire to compensate for it: the two streams are
joined by a ring buffer rather than by graph links, so there is no graph latency to
adjust, and the figure is there so the number is visible rather than guessed at.

**One dependency needs one warm-up.** Noise reduction calls `nnnoiseless`, which
keeps its window and DCT tables in a process-wide `OnceCell` that is filled on
the first `process_frame`. `NoiseReduction::new` runs one frame of silence
through a throwaway `DenoiseState` so that the cell is filled when the effect is
built on the control thread rather than in the audio callback. That call is
allocation-free afterwards and is the only place a dependency is warmed up.

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
- **The pitch shifter is a phase vocoder.** It recovers each bin's instantaneous
  frequency and remaps the bins, so it smears transient attacks and costs a
  2048-frame analysis window: 42.7 ms of declared latency at 48 kHz, reported but
  not compensated. It does not preserve formants, and the reference's SoundTouch
  modes have no equivalent here.
- **Bass loudness approximates the contour with two shelves.** A low shelf at
  120 Hz and a high shelf at 6 kHz whose gains grow as the source level falls,
  rather than MDA's own transfer function, which is not verifiable from this
  checkout.
- **The GPU stage is synchronous.** Each off-loaded block uploads, dispatches,
  waits and downloads inside the callback. The wait is capped at half the block
  duration and a miss falls back to the CPU, so the stage can never consume the
  whole deadline. A device-resident chain would remove the copies and the wait,
  but would require every effect to keep its state on the device; that is a
  different architecture, not a flag.
- **Only elementwise stages are off-loaded.** Recursive filters and spectral
  effects cannot be moved one block at a time without uploading their state.
  The setting is honest about the subset it accelerates.
- **Noise reduction is fixed at 48 kHz.** RNNoise is defined on 480-sample
  frames at 48 kHz, and so is `nnnoiseless`. At any other graph rate the stage
  passes the signal through unchanged rather than resampling it: a resampler in
  the callback would add latency and its own realtime budget, and the reference
  pays for a Speex resampler to avoid that limit. `docs/feature-matrix.md`
  records the reduction.
- **The FFT inside that dependency is planned on first use.** `nnnoiseless`
  reaches its transforms through `easyfft`, which keeps a planner and a scratch
  buffer per transform length in a thread-local, and `nnnoiseless` itself keeps
  its window tables in a process-wide cell. The cell is warmed up on the control
  thread (see section 3); the thread-local cannot be, because the audio thread
  cannot be reached from outside. The cost is one planned 960-point transform and
  one scratch buffer on the thread that first runs the stream callbacks, once for
  that thread and not per block, and it is the only allocation this project's
  audio path can still make. Removing it needs an API in the dependency that
  prepares a state for a given thread.

## 7. The interface

`adw::ApplicationWindow` → `adw::OverlaySplitView`. The end side is an
`adw::ToolbarView` holding the header, the banner and an `adw::ViewStack`; the
start side is a `gtk::Stack` that shows one sidebar per pipeline. The `ViewStack`
has two titled children, **Output** and **Input**, and its `adw::ViewSwitcher` is
the header's title widget, so the header carries no window-title text. The two
pipelines have `Ctrl`+`1` and `Ctrl`+`2` accelerators, registered as window
actions. Below 860 px the split view collapses: the editor fills the window, the
switcher switches to its narrow policy, and the sidebar opens as an overlay
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
  *back* button that closes the drawer; below it is the effect list, then a
  divider, and below that the preset actions row (save, import, export, delete,
  duplicate) sitting above the preset list. A pipeline with no preset of its own
  and no skipped file shows a one-line empty state instead, and skipped files are
  listed in full in a *Skipped files* row. Each preset row carries a round check
  button that loads that preset; the button on the loaded preset is accented and
  unloads it, which empties the chain and leaves no preset active.
- **Effects toggle**: the chain on/off, a labelled toggle button at the top of the
  sidebar. It is accented while that pipeline's chain processes audio and plain
  while it is bypassed, which is the state a user reads at a glance. It replaced
  an icon-only control whose meaning was the opposite (an unchecked *Bypass*
  switch), so a running chain looked switched off. Each pipeline has its own
  toggle, and it drives that pipeline's bypass.
- **Editor and meters**: the end side carries the chain's input and output
  meters and the live spectrum above the editor for the selected effect.
- **Devices** (menu): the format and quantum of the active pipeline, its node
  name with a copy button, its routing hint, its destination chooser, a
  *Microphone* switch and a *Reconnect* button that appears when the engine has
  failed.
- **Preferences** (menu): *Start on login*; a *Processing backend* chooser with
  *CPU*, *GPU* and *Both* (the default), which restarts the running pipelines on
  the selected backend; an *Auto-save preset edits* switch with an *Update
  interval* chooser; and a *Developer mode* switch. When auto-save is on, an
  edit to the loaded preset is written back to that preset after the chosen
  interval has passed without further changes. *Developer mode* adds a
  *Diagnostics* entry to the main menu; the entry is absent while the switch is
  off. The Diagnostics dialog carries the engine counters, the detected Vulkan
  compute devices, the GPU block/fallback/disagreement counters and the
  diagnostic messages. Every choice is persisted.
- **Chain rows**: one flat line per effect, in processing order - the effect's
  symbolic icon, its name, an on/off `gtk::Switch` and the row menu. The icon is
  drawn in the theme's foreground colour, not the accent, and dims while the
  effect is disabled, as does the name; the switch carries the accent and is the
  enable control. Clicking the row selects it for the editor; the summary is the
  row's tooltip, and move up/down and remove are behind a menu button that is
  always visible; a secondary click opens the same menu. The rows
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
- **Spectrum**: a `gtk::DrawingArea` under the meters draws the 48 band levels of
  the audio leaving the chain as bars on a logarithmic axis, with 100 Hz, 1 kHz
  and 10 kHz grid lines and a caption naming the loudest band and its level. The
  bars are drawn in the same data colour as the equalizer's response curve and
  the grid in the widget's own foreground colour, so both are legible in either
  appearance. The band levels are a real measurement of the processed audio,
  recomputed every third block by the pipeline's own analyser and read from an
  atomic snapshot by the 50 ms poll; a pipeline with no engine, and an engine
  that has been stopped or shut down, reads the floor and the caption is empty
  rather than the last frame that happened to be measured.
- **Banner**: engine errors and conversion warnings appear in an `adw::Banner`
  with a *Dismiss* button. The meter's hold and RMS markers and the equalizer's
  band markers are drawn from the widget's own foreground colour, so each stays
  visible in both appearances; a fixed colour made the hold and RMS markers
  invisible in one appearance and the band markers invisible in the light one.

### How the spectrum is measured

- The analyser keeps 8192 samples of the mid signal (`0.5 · (left + right)`), so
  a stereo source is measured the way a listener hears it. The size is the
  reference's own default, and it is what makes the band layout meaningful: with
  the first version's 1024-point transform, 22 of the 48 bands sat below the
  window's resolution and reported one shared bin under nine different frequency
  labels.
- It applies a Hann window and the FFT of `crates/dsp/src/fft.rs`, then converts
  each bin to power with the window-energy correction
  `1 / (N · Σw²)`, so the bin powers sum to the signal's mean square.
- A band's level is the integral of that power between its edges, using a
  cumulative table and linear interpolation, not the peak bin inside it. Two
  tones inside one band therefore report the sum of their powers, which is what
  the test `a_band_sums_every_tone_it_contains` pins.
- A band narrower than the window's resolution bandwidth (`rate · Σw² / (Σw)²`,
  which is 8.8 Hz for this window at 48 kHz) is multiplied by
  `resolution_bandwidth / width`, exactly as the reference does. That normalises
  the seven lowest bands to the analysis bandwidth instead of pretending they can
  be separated; the test `a_tone_below_the_resolution_is_still_reported_at_its_level`
  pins that such a tone still reads its own level.
- The published figure is `10 · log10(4 · power)`, which for a one-sided spectrum
  is the squared amplitude of a sinusoid carrying that band's power. A
  full-scale tone therefore reads 0 dB, a half-scale tone −6 dB, and the spectrum
  agrees with the peak meters next to it. The bands run from 20 Hz to 20 kHz on a
  logarithmic axis, and a band above the graph's Nyquist is left at the floor
  rather than clamped to it.

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
- **A preset can be unloaded.** The check button on the loaded preset clears the
  chain and leaves no preset active, so an empty chain is a first-class state
  rather than one that can only be reached by removing every effect by hand.
- **A preset can be duplicated.** *Clone* writes a copy of the loaded preset,
  both sections, under a new name in the user directory, without changing the
  live chain.
- **The loaded preset offers an update.** Each preset row shows its name in bold
  with its origin (`Built in`, `User preset`, `System preset`) beside it on the
  same line. The loaded preset's row always carries a menu with *Update preset*,
  which writes the current chain back, and *Revert changes*, which reloads the
  stored preset.

Built-in presets are compiled in, so a fresh installation has a usable set
without a system preset directory. There are six: *Default*, *Podcast voice*,
*Music, speakers*, *Headphones*, *Bass boost* and *Clean microphone*. Each is a
JSON file under `data/presets/`, `make install` writes it into the system
directory under its preset name — the store takes a preset's name from the file
stem, so the installed file name has to be the preset name — and the tests in
`crates/presets/src/builtin.rs` assert that every built-in parses, that it names
only effects the registry has, and that it sets only parameters those effects
declare.

## 9. Settings, the command line and packaging

`pwe-services` owns one document, `$XDG_CONFIG_HOME/pipewire-effects/settings.json`:

```json
{
  "output": { "device": "default", "bypassed": false },
  "input":  { "device": "default", "bypassed": false },
  "preset": "Default",
  "process_input": false,
  "autostart": false,
  "developer_mode": false,
  "auto_save": false,
  "auto_save_interval": 10,
  "audio_backend": "both"
}
```

It is written atomically through a temporary file, and a damaged document falls
back to the defaults with a message instead of stopping the application. The
document is what makes the device choice and the loaded preset survive a restart;
previously both were lost.

`pwe-services` also owns the autostart entry. *Start on login* writes
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

- `cargo test --workspace`: 466 tests, no audio device needed.
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
- The limiter is checked as a lookahead limiter and not as a clipper: with a
  20 ms lookahead a full-scale burst must keep its sine shape, which is asserted
  as a crest factor within 0.03 of √2 (a clipped sine drops below it) with the
  peak sitting on the ceiling; the output must lag the input by exactly the
  lookahead; the ceiling must still never be exceeded with 20 dB of drive; and a
  0 % stereo link must limit the loud channel without touching the quiet one.
- The chain's latency is the sum of the enabled stages' declared latency, and the
  processor publishes it with the other counters; the limiter reports its
  lookahead in frames.
- The noise reduction is checked for the frame its dependency defines: a rate
  other than 48 kHz passes the signal through unchanged, the first frame of the
  network is the dry input, a zero mix is transparent, the output gain scales the
  result, and the output stays finite under abuse. It is also the one effect with
  its own live test: `crates/engine/tests/live_noise_reduction.rs` runs it in the
  real chain on a private sink, asserts that a −20 dB tone arrives at its own
  level and leaves more than 20 dB lower, and asserts that the underruns stay
  below an eighth of the processed frames. Two verification runs measured
  12 544 and 14 592 frames processed, each with 512 underrun samples from the
  initial fill and no overruns or trims, and the tone leaving 124.00 dB and
  87.03 dB lower.
- The sidechain filter is checked on its own: a 400 Hz high-pass must cut a 60 Hz
  tone by at least a factor of five, a 36 dB/oct slope must cut more than a
  12 dB/oct one, and both must leave a 4 kHz tone within 0.05 of untouched.
- The compressor's sidechain is checked end to end: a 60 Hz tone driven hard must
  be compressed with the filter off and left nearly alone with a 400 Hz high-pass
  engaged.
- The reverberation's tone stage is checked by comparing two tails: a
  high-pass at 1 kHz and a low-pass at 1.5 kHz must leave the tail below 90 % of
  the energy a wide-open setting leaves.
- The stereo tools are checked control by control: the mono-sum and channel-swap
  modes, the mid/side round trip in both directions, middle and side level
  acting independently, output balance, per-channel phase and mute, soft clip
  holding a hot signal below unity, and the Haas delay landing the right
  channel's peak where it should.
- The spectrum analyser is checked against tones and levels, and against a
  discontinuity between blocks. A 997 Hz tone must land in a band whose centre is
  within 250 Hz of it and read its −6.02 dB level to within 0.6 dB, a full-scale
  tone must read within 0.6 dB of 0 dB, two tones inside one band must sum to
  +3.01 dB rather than reporting one of them, a tone below the analysis
  resolution must still read its own level, silence must stay at the floor, a
  band above the Nyquist of an 8 kHz graph must stay at the floor, at most a
  quarter of the bands may sit below the analysis resolution, a quantum shorter
  than the transform must report nothing until the history is full, and the band
  centres must rise and span the audio range. The signal generator keeps its
  phase across blocks, because a tone that restarts every block is not a tone:
  that mistake made an earlier version of these tests pass by luck.
- The engine publishes that measurement rather than a stored value: a full-scale
  1 kHz tone pushed through the realtime processor must appear in the loudest
  band, within 250 Hz of its frequency and within 3 dB of its own level; the
  snapshot object is checked for returning what was published and for returning
  every band to the floor when it is cleared, which is what the engine does when
  it stops.
- The reverberation pre-delay is checked for channel isolation: an impulse in the
  right channel must not appear at the left output, which fails if the two
  channels share one pre-delay line.
- The registry carries each descriptor's parameter specifications, so a
  descriptor and its parameter table cannot drift apart.
- Every effect's symbolic icon is checked against `data/`: the file named by
  `format::effect_icon` must exist, must draw from `currentColor`, must be a
  16 px symbolic icon, and must keep each tag on one line. The line-break rule is
  cosmetic — `rsvg-convert` and the gdk-pixbuf SVG loader were measured to render
  the two spellings byte-identically — so the test enforces a formatting
  convention, not a rendering requirement.
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
  virtual source exists and that the recording drove the input chain. Four of
  the nine live tests are these; `crates/engine/tests/live_gpu.rs` adds one for
  `GPU` and one for `Both`, described in section 12,
  `crates/engine/tests/live_noise_reduction.rs` adds the one below, and
  `crates/engine/tests/live_pitch_shift.rs` and
  `crates/engine/tests/live_bass_loudness.rs` add the two described below. All
  nine are `#[ignore]`d so they do not run by default, and they serialise on a
  process-wide lock because they mutate the shared graph.
- `crates/engine/tests/live_noise_reduction.rs` puts the noise reduction stage in
  the real chain on a private sink: it asserts that a −20 dB 1 kHz tone reaches
  the chain at its own level, that the chain processed at least 4 800 frames, that
  the stage took more than 20 dB off the tone, and that the underruns stay below
  an eighth of the processed frames. It samples the meters across the whole run and
  keeps the largest reduction it sees, because the network takes time to settle
  and a single sample taken as the tone starts is not the steady-state figure:
  three consecutive runs measured the tone arriving at −20.00 dB and leaving
  124.00 dB, 96.14 dB and 50.52 dB lower. Before that change the test read the
  meter once and failed intermittently on a loaded machine, which is what a
  single-sample measurement of a settling stage does. A steady sine is noise to a speech model, which is why the
  reduction is that large; the point of the test is that the network runs inside
  the realtime callback without dropping audio, which is the risk `nnnoiseless`'
  lazily planned transform carries into the first frame.
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
  `chain`, `editor`, `equalizer`, `presets`, `modified`, `preset-menu`,
  `unloaded`, `clone`, `input`, `devices`, `preferences`, `developer`, `gpu`,
  `menu`, `menu-developer`, `dev-live`, `picker`, `picker-tone`, `about`, `save`,
  `delete`, `banner`, `bypass`, `narrow`, `mobile`, `mobile-drawer`,
  `sidebar-hidden` and `effect:<kind>`, which loads the built-in preset, adds the
  named effect if it is absent and selects it. `devices`, `preferences` and
  `diagnostics` activate the `win.devices`, `win.preferences` and
  `win.diagnostics` actions, so the menu wiring is exercised too, and `dev-live`
  flips the real *Developer mode* switch the way a pointer would. This is the
  harness the interface review in section 13 used; it is not part of the product.
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
- `TONE=1 scripts/capture-screen.sh` plays a −20 dB 440 Hz sine into the
  application's own sink with `ffmpeg` and `pw-cat` while it captures, so the
  meters and the spectrum are photographed with real audio flowing rather than at
  the floor. It is how the spectrum was checked against the output meter:
  `441 Hz −44.1 dB` in the caption against `−44.0 dB` on the meter, with the
  tone's own file measured at −41.1 dB and the input meter reading the same.
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

`cargo test --release -p pwe-dsp --test benchmark -- --ignored --nocapture` times
each effect and the full chain on the machine it runs on — twenty-four effects
when this table was taken; the paragraphs after it record the twenty-six and
then the twenty-seven-effect runs: a 48 kHz stereo stream, a per-block wall clock over 2 000 blocks, three rounds per
workload of which the fastest is reported so that concurrent desktop load cannot
inflate a figure, and the tone-generation cost subtracted so the numbers are the
effects. It also times the spectrum analyser, which runs in the audio path, and
fails if the full chain ever needs more than half of a block. Three consecutive
runs on the development machine (Intel Core i7-7500U, two cores and four threads,
`rustc 1.99.1`, release profile with thin LTO, concurrent desktop load from
Firefox and GNOME Shell) gave:

| Workload | 1024 frames per block |
|---|---|
| All twenty-four effects | 2.40–3.57 ms/block · 6.0–8.9× realtime · 11.2–16.7 % of one core |
| Noise reduction (heaviest) | 0.66–0.80 ms/block · 3.1–3.8 % of one core |
| Voice suppressor | 0.48–0.51 ms/block · 2.2–2.4 % of one core |
| Multiband compressor | 0.28–0.38 ms/block · 1.3–1.8 % of one core |
| Multiband gate | 0.26–0.35 ms/block · 1.2–1.6 % of one core |
| Equalizer | 0.09–0.17 ms/block · 0.4–0.8 % of one core |
| Reverberation (with its tone stage) | 0.08–0.18 ms/block · 0.4–0.8 % of one core |
| Expander | 0.025–0.083 ms/block · 0.1–0.4 % of one core |
| Compressor | 0.048–0.059 ms/block · 0.3 % of one core |
| Limiter (5 ms lookahead) | 0.041–0.059 ms/block · 0.2–0.3 % of one core |
| Gate (6 dB curve) | 0.025–0.026 ms/block · 0.1 % of one core |
| Stereo tools | 0.011 ms/block · 0.05 % of one core |
| Spectrum analyser | 0.117–0.194 ms/block · 0.55–0.91 % of one core |
| Gain (lightest) | at or below the timer's resolution |

The whole chain finishes between a sixth and a twelfth of the 21.3 ms deadline
and leaves 6.0–8.9× realtime of headroom, the spread being the desktop's own
load rather than anything in the chain: on the same machine and in the same
session the figure moved by 45 % between runs while the per-effect figures moved
by less. The heaviest stages are the ones that work in the frequency domain:
`nnnoiseless` runs a 960-sample transform and a recurrent network per 10 ms
frame, the voice suppressor runs a 1024-point STFT with 75 % overlap on every
block, and the spectrum looks at 8192 samples every third block. The small
effects read as zero in a loaded run because they are cheaper than the timer's
resolution; that is a limit of the measurement, not a claim that they are free.

**The chain had grown to twenty-six effects at that point, and the same test measured
it.** A later run on the same machine with the same method, after pitch shift and
bass loudness were added, reported 3 196 μs per 1024-frame block for all
twenty-six effects, 6.7× realtime and 15.0 % of one core. Pitch shift on its own
cost 789 μs (3.7 % of one core) and bass loudness 16 μs (0.08 %). The pitch
shifter is the second most expensive stage in the chain: it runs four 2048-point
transforms per 1024 frames, and its buffers, window and twiddle tables are
allocated when the effect is built, never in the callback. The chain still
finishes inside a seventh of the deadline, but it is no longer the 6.0–8.9×
headroom the table above records.

After the loudness compensator, the crystalizer's thirteen bands and the gate's
sidechain were added, a run at a load average near three measured 4 604.60 μs per
1024-frame block for the twenty-seven-effect chain, 4.6× realtime and 21.6 % of
one core. The loudness stage costs 30.94 μs (0.15 %) and the crystalizer rose
from 57 μs to 143.65 μs (0.67 %), because it now runs thirteen low-pass sections
per sample instead of three.

The speech processor, autotune and the harmonizer were added after that, and the
same test now measures the thirty-effect chain at 6 969.50 μs per 1024-frame
block: 3.1× realtime and 32.67 % of one core. The two new pitch stages are the
reason for the increase, because autotune and the harmonizer each run the
2048-point vocoder and the harmonizer runs two of them, so the chain now pays
roughly twice what a single pitch shift costs, on top of the pitch shifter that
was already there. It still finishes inside a third of the 21.3 ms deadline, but
the headroom is now 3.1× rather than the 6.4× the previous run recorded. The
per-effect figures move with the load on the machine, so a re-measurement is the
only way to reproduce them on other hardware.

**The sidechain filters bypass rather than compute.** Running three high-pass and
three low-pass biquads per channel per sample whether or not they are engaged
cost the compressor 111 μs per block against 58 μs once a disabled filter
short-circuits to the raw sample, and the expander 215 μs against 83 μs; both
figures are from the same machine and the same test, with both filters off in
both cases, which is the state a preset written before this change loads in. The
reverb's tone stage cannot take that shortcut because it is doing real work, and
it shows: 0.08–0.18 ms against 0.08–0.10 ms before.

**The measurement method changed with this table.** The earlier figures were one
run per workload, which under concurrent load produced a range of 1.7–6.3 ms for
the full chain; this table reports the fastest of three rounds instead, so its
numbers are lower for the same work and the two tables are not comparable. Re-run
the measurement to reproduce a figure on other hardware, and read it together
with the load on the machine.

The final verification pass re-ran the same test at a load average of 3.08 and
reproduced the figures: all twenty-six effects as the chain stood then 3 358.92 μs per 1024-frame block,
6.4× realtime and 15.7 % of one core; pitch shift 833.46 μs (3.9 %); noise
reduction 674.29 μs (3.2 %); voice suppressor 522.91 μs (2.5 %); multiband
compressor 317.69 μs (1.5 %); spectrum analyser 116.98 μs (0.55 %); bass loudness
29.83 μs (0.14 %); gain 4.40 μs (0.02 %). Bass loudness read 29.8 μs here against
16 μs recorded above, which is the spread of a sub-30 μs measurement on a loaded
machine rather than a regression: it is four biquads per sample and moves with
cache and scheduling state. The per-effect numbers are reproducible to that
order; the chain total is reproducible to within about 5 %.


### Processing backend selection (CPU, GPU, Both)

`Preferences` → `Audio processing` → `Processing backend` offers `CPU`, `GPU`
and `Both`. The choice is stored as `audio_backend` in
`$XDG_CONFIG_HOME/pipewire-effects/settings.json`, applies to both pipelines,
and is re-applied when a pipeline is started, retargeted or reconnected. It is
a real processing selection, not a UI state: changing it restarts the running
engines and rebuilds their chains against the selected backend.

- **`CPU`** runs every effect on the audio thread. It is no longer the
  built-in default: a fresh install selects `Both`, which keeps the GPU stage
  and its CPU reference. A machine without a Vulkan device falls back to `CPU`
  with a warning.
- **`GPU`** runs the elementwise gain stages on a Vulkan compute device. A
  gain stage is exactly the set of effects that expose
  `Effect::elementwise_gain`, currently `gain`. Filters, dynamics, reverbs and
  spectral effects keep per-sample recursive state and stay on the CPU; moving
  them would require a device-resident chain and a different graph topology.
- **`Both`** executes the same GPU stage and also computes the CPU reference
  for each block. A block whose GPU result differs by more than
  `max(1e-5, 1e-5 · |reference|)` is replaced by the CPU result and counted in
  `gpu_disagreements`.

The realtime contract for the GPU stage is fixed:

1. The Vulkan instance, physical device, queue, buffers, descriptor set,
   pipeline and shader are created once when the engine starts, and a warm-up
   dispatch runs there, never in the audio callback.
2. Per block the stage copies host-mapped samples in, records and submits one
   dispatch, waits for the fence, and copies the result out. The wait is
   bounded by **half of the current block duration**, computed from the frame
   count and sample rate, with a 100 µs floor and a 25 ms ceiling. It never
   allocates, locks or logs in the callback.
3. If the fence misses that deadline, or the device reports an error, the CPU
   applies the gain for that block and the `GPU fallbacks` counter increments.
   Audio is still correct. Three consecutive deadline misses, or any Vulkan
   error, latch the accelerator off for that engine and the chain continues on
   the CPU.
4. If no Vulkan compute device exists, selecting `GPU` or `Both` reports a
   warning and keeps the CPU selection; a configuration file that names a GPU
   backend on another machine degrades to CPU with a warning at engine start.

Measured on the development machine (Intel Core i7-7500U, Intel HD Graphics
620, `rustc 1.99.1`, release): one 1024-frame stereo gain block costs 0.17–0.22 µs
on the CPU and 140–330 µs through the persistent GPU stage depending on desktop
load; a one-shot GPU call including device setup costs 11–29 ms. The GPU path is
therefore slower than the CPU on this hardware. It is offered because it is a real backend the user can
select and verify, not because it improves throughput; the application never
claims GPU acceleration as a performance win.

The live tests `crates/engine/tests/live_gpu.rs` run the real daemon, a
private virtual sink and `pw-cat`, once for `GPU` and once for `Both`. They
assert that processed frames reached the chain, that GPU blocks were counted,
that deadline fallbacks stayed at most a tenth of the blocks, and that `Both`
reported zero disagreements. These are `#[ignore]`d like the other live tests
and are run explicitly. The decision and its evidence are pinned here so a
later change cannot silently turn the setting into a mock or delete it.

**The fallback ratio is a property of the machine's scheduling, not of the
GPU's throughput, and the two live tests fail on a busy one.** Measured on this
development machine: the persistent GPU stage costs 302.5 μs for a 1024-frame
stereo block (`cargo test --release -p pwe-engine --lib gpu -- --ignored
--nocapture`), against a fence budget of half of a 1024-frame block at 48 kHz,
which is 10.7 ms — thirty-five times the cost of the stage itself. A fallback
therefore does not mean the GPU is slow, it means the driver or the scheduler
stalled past the budget that block. When the session's default output was the
built-in analogue device and the machine was quiet, the same tests measured 0
fallbacks in 44–52 GPU blocks. With a Bluetooth headset as the session default
and a load average of 3–6 on this two-core, four-thread machine, they measured
4–9 fallbacks in 30–75 blocks — 11 % to 17 %, above the tenth the test asks for —
and the two tests reported `FAILED` while `gpu_disagreements` stayed at 0 in
every run, which is the point: a missed deadline applies the gain on the CPU, so
the audio stays correct and the counters say what happened. Nothing in the GPU
backend was changed in the pass that observed this; the throughput figure above
is the same as it was. Treat a failure of those two tests as evidence about the
machine first, and re-run them on a quiet one before concluding that anything in
`crates/engine/src/gpu` regressed.

**The fence budget follows the negotiated graph quantum, not a fixed block size.**
`Chain::process` computes it as `block_budget_ns(buffer.frames(), rate)`, half of
the block the callback was actually handed. A session that negotiates 1024-frame
quanta gives the stage 10.7 ms; the final verification pass on the development
machine ran at a **256-frame** quantum, which the application's own Devices page
reported as `256 frames · 5.3 ms`, and the budget there is 2.67 ms. That run
measured 5 fallbacks in 46 GPU blocks (10.9 %) for `GPU` and 4 in 28 for an
earlier attempt, with `gpu_disagreements` at 0 in every run, at load averages of
2.3 and 5.3 on a four-thread machine whose default output was a Bluetooth
headset. The two live tests assert at most a tenth, so they reported `FAILED`
while the audio stayed correct and the counters said what happened. The stage
itself cost 169.4 μs for a 1024-frame stereo block in the same pass
(`cargo test --release -p pwe-engine --lib gpu -- --ignored --nocapture`), which
is 63 times inside the 10.7 ms budget and 16 times inside the 2.67 ms one, so the
fallbacks are scheduler stalls rather than a slow kernel. Read the two live tests
together with the session's quantum and its load; neither figure is a property of
the backend alone.

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
  the menu button is now drawn on every row instead of appearing on hover, so the
  reorder and remove actions are reachable without a pointer gesture.

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
`po/pipewire-effects.pot` (504 messages), and
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
the layout adaptive: the split view collapses below 860 px, the switcher switches
to its narrow policy, a back button closes the drawer, and the window's minimum size is 360x480.
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

The backend-setting pass added the *Processing backend* chooser and verified it
on the running application rather than in a mock. A private
`XDG_CONFIG_HOME` containing
`{"audio_backend":"both","preset":"Default","developer_mode":true}` was used to
launch the release binary; the *Preferences* capture showed `Both` selected
before any interaction, and the diagnostics capture read 677 GPU blocks, 1
fallback and 0 disagreements on the Intel HD 620, which is the proof that the
saved setting reaches engine construction and the real processing path. The same
path was exercised with `GPU` and `Both` through
`crates/engine/tests/live_gpu.rs` against the live daemon and a private virtual
sink: 37–47 GPU blocks per run, at most one deadline fallback in ten runs, and
zero disagreements. The built-in *Default* preset gained a transparent 0 dB
`gain` stage so the elementwise path exists on a fresh installation; without it
the GPU counters correctly stayed at zero and the choice looked inert.

The same pass captured `preferences` and `gpu` in the light and the dark
appearance, plus `save`, `delete`, `modified`, `preset-menu`, `chain`, `presets`,
`devices`, `about`, `input`, `narrow`, and the phone layouts. The save button in
the preset action row no longer carries a filled rounded background: it is a
flat button whose symbolic glyph is drawn in the theme accent colour, matching
the red delete glyph next to it. The harness was also corrected in this pass:
`modified` and `preset-menu` used to add a second `gain` stage after the built-in
preset had gained one, and they now add a `delay` instead, which is what makes
the loaded preset modified. The `preset-menu` capture from
`scripts/capture-ui.sh` cannot show the popover itself because a `GtkPopover` is
a separate surface; it shows the row with its menu button active.
`scripts/capture-screen.sh` takes the whole X root window under `Xvfb`, and its
`preset-menu` PNG shows the open popover with *Update preset* and *Revert
changes*, so the menu is verified by a capture rather than by wiring alone.

No PipeWire daemon, device profile, default route or system setting was changed
by these captures themselves. The application's virtual nodes keep
`node.virtual = true`, `priority.session = 0`, `state.restore-props = false` and
`state.restore-target = false`, and its engine ignores a `default.audio.sink`
value that names one of its own nodes. During this pass the session's
`default.audio.sink` metadata was found pointing at `pipewire-effects-sink`,
left there by an earlier session; the real built-in device was restored with
`wpctl set-default` and the live tests were then re-run, after which the default
stayed on the real device and the app's sink appeared only as a lower-priority
fallback entry. The rule for future sessions is recorded in `AGENTS.md`: check
`pw-metadata -n default | grep default.audio.sink` after a run and never leave an
internal node as the default.

The final pass moved the diagnostics out of Preferences and made the processing
backend default to `Both`. *Developer mode* in Preferences now only controls
whether a *Diagnostics* entry exists in the main menu; the entry is a
`gio::MenuItem` with the `hidden-when = action-disabled` attribute and the
`win.diagnostics` action is enabled from the persisted developer-mode setting,
so the menu can never show a dead entry. The counters, the detected compute
devices and the messages live in the `Diagnostics` dialog
(`crates/ui/src/diagnostics.rs`, `crates/ui/src/pages.rs`) and the Preferences
page no longer carries them. The menu was captured with
`scripts/capture-screen.sh`: `menu` shows *Devices*, *Preferences*, *About
PipeWire Effects* and *Quit* with no *Diagnostics*, and `menu-developer` shows
*Diagnostics* at `Ctrl`+`5`; the `developer` capture shows the dialog itself
with the live counters (`target/ui-screens-light/menu-prefer-light.png`,
`menu-developer-prefer-light.png`, `developer-prefer-light.png`). The
`Preferences` capture in the dark appearance shows *Both* as the selected
backend and *Developer mode* with the subtitle *Adds a Diagnostics entry to the
main menu*, and no counters on the page.

Toggling *Developer mode* was checked through the real widget, not by calling
the state setter: the `dev-live` harness state opens Preferences, flips the
`Developer mode` switch the same way a pointer would, closes the dialog and
opens the main menu. The capture
(`target/ui-screens-light/dev-live-prefer-light.png`) shows *Diagnostics* with
`Ctrl`+`5` in the menu, which is the interactive path the earlier screen misses.
The first version of the change enabled the `win.diagnostics` action only when
the chain signature changed, so the entry stayed hidden until an unrelated
refresh; `Ui::refresh` now sets the action from the persisted developer-mode
value on every 50 ms tick, and the `dev-live` capture is the regression check
for that fix.

### The noise reduction and spectrum pass

The pass that added the twenty-fourth effect and the spectrum re-ran the harness
on the headless Wayland session, in both appearances: the twenty-four states of
`capture-ui.sh` (the standard set plus `effect:noise_reduction` and `input`) in
the light appearance, and the standard set plus the noise reduction editor in the
dark one, followed by `TONE=1 capture-screen.sh` on the X11 backend in both
appearances so that the meters and the spectrum were photographed with a real
tone flowing. What it found and changed:

- The spectrum's caption named a frequency the transform could not resolve. With
  the first version's 1024-point transform the bin width at 48 kHz is 46.9 Hz, so
  22 of the 48 bands lay below the analysis resolution and nine of them reported
  one shared bin under nine labels from 21 Hz to 79 Hz; the capture read
  `21 Hz −39.2 dB` for content at 47 Hz. The analyser now uses the reference
  analyser's own default 8192-point transform, integrates the Parseval-corrected
  bin power over each band instead of taking one peak bin, and normalises a band
  narrower than the window's resolution to the analysis bandwidth. The same
  capture now reads `441 Hz −44.1 dB` for a 440 Hz tone whose output meter reads
  `−44.0 dB`, and the microphone capture reads `91 Hz −40.4 dB` rather than a
  frequency the transform cannot measure.
- The effect list of the picker and the chain render the new effect with its own
  icon in both appearances; the noise reduction editor renders its two groups and
  its five controls with no clipping at 1040x720, at 720x620 and at 360x740.
- The analyser's own tests were generating their signals with the phase restarting
  every block, which makes the "tone" a periodic signal with a 1024-sample period
  rather than a tone. Two level assertions passed by luck until the generator was
  given a running phase.
- A reference RNNoise preset's `wet` key was being listed as handled and dropped
  while the stage has an equivalent mix control; it is transferred now, and the
  keys with no equivalent are reported. The conversion tests cover both.

The session's `default.audio.sink` was found pointing at the application's own
node after this pass, as `docs/architecture.md` records happened before: the
stale `default.configured.audio.sink` from an earlier experiment makes the
application's sink the session default for as long as it runs. The real
built-in device was restored with `wpctl set-default` and both metadata keys
verified against it afterwards. The application neither writes nor reads that
key, and making it the default output is a documented way to use it, so nothing
in the application changed because of this; the check is a session-hygiene rule
and `AGENTS.md` and `scripts/README.md` both carry it.

### The pitch shift and bass loudness pass

The pass that added the twenty-fifth and twenty-sixth effects re-ran the harness
on an isolated headless GNOME Wayland session and exercised both new editors and
the live path. What it captured, and what the captures and logs showed:

- 25 light states (`chain`, `editor`, `equalizer`, `presets`, `modified`,
  `unloaded`, `devices`, `preferences`, `developer`, `gpu`, `picker`, `about`,
  `banner`, `bypass`, `narrow`, `mobile`, `mobile-drawer`, `sidebar-hidden`,
  `input`, `effect:bass_loudness`, `effect:pitch_shift`,
  `effect:noise_reduction`, `effect:stereo_tools`, `effect:reverb`,
  `effect:crystalizer`) and 19 dark states, all written by the application's own
  render tree. No state failed to produce a PNG.
- Both new editors render their hero row with a distinct symbolic icon, their
  groups and all their controls, in both appearances. The pitch editor shows
  *Semitones* as `+0.00 st` and *Cents* as `+0 ct`, which is the display of the
  two new `ParamUnit` variants; the loudness editor shows *Loudness* under
  *Contour* and *Output* and *Link* under *Output*. Neither clips at 1040x720,
  720x620 or 360x740, and both appear in the effect picker under *Tone* with
  their summaries.
- The narrow capture shows the expected collapsed layout: the content fills the
  window, the switcher is in its narrow policy form (icon above label, not
  strictly icon-only), and the drawer holds the chain and the preset list.
- `TONE=1 scripts/capture-screen.sh` captured the chain with a −20 dB 440 Hz sine
  flowing through the virtual sink on the X11 backend. The input meter read
  `−41.1 dB` and the output meter `−44.0 dB`, and the spectrum's caption read
  `441 Hz −44.1 dB`, so the analyser and the output meter agree on the tone's
  frequency and level in a real capture, not only in the unit tests.
- The `input` capture read `−36.8 dB` at the input meter and `−36.9 dB` at the
  output meter with a broadband spectrum, which is the real capture device
  feeding the second chain.
- The `gpu` capture (Diagnostics, backend `Both`) read 145 GPU blocks, 20
  deadline fallbacks, 0 disagreements and `Intel(R) HD Graphics 620 (KBL GT2)
  detected`, with a chain latency of 240 frames · 5.0 ms (the *Default* preset's
  5 ms limiter lookahead). That is the proof that the persisted backend reaches
  engine construction and that the accelerator runs in the real application.
- The live suite passed on the daemon: `live_pipewire` 4/4 (a −20 dB tone
  measured at the input and −40 dB at the output, and the input chain at
  `−36.85 dB` in and `−42.85 dB` out over 10 240 processed frames),
  `live_noise_reduction` (201 472 frames, tone leaving 96.14 dB lower),
  `live_pitch_shift` (a 1 000 Hz tone's loudest band moved from 1 047 Hz to
  2 149 Hz at +12 semitones over 184 320 processed frames, 512 underruns from
  the ring's initial fill, no overruns or trims) and `live_bass_loudness`
  (a 60 Hz tone arrived at −20.00 dB and left at −8.15 dB, an 11.85 dB lift over
  113 152 processed frames). Every run reported 0 overruns and 0 trims.
- The CPU baseline and the GPU block timing were re-measured and are recorded in
  section 12; the two GPU live tests reported `FAILED` at the tenth-of-blocks
  threshold under load and at a 256-frame quantum, which section 12 explains.
- `make install DESTDIR=<staging> PREFIX=/usr` installed the binary, the desktop
  entry, 27 symbolic icons (both new ones included), the application icon and
  the six built-in presets; `desktop-file-validate` accepted the entry and
  `make uninstall` removed exactly what was installed. `scripts/update-po.sh`
  regenerated a 450-message template and the xgettext lifetime warnings were
  confirmed harmless by checking that messages in the affected functions are
  present in the template.
- The window smoke test (`PIPEWIRE_EFFECTS_SMOKE_TEST=1`) reported
  `state=Running devices=8 rate=48000 failures=0` and exited 0.

The pass also found and corrected one session-hygiene problem before it could
affect the captures: `default.configured.audio.sink` still named
`pipewire-effects-sink` from an earlier experiment, so the application's own sink
would have become the session default as soon as an instance started. It was
restored to the real `bluez_output.AC_1F_37_E5_36_35.1` device with
`wpctl set-default 157`, and both metadata keys were verified against it before
any capture ran. No device profile, stored volume, default route or system
setting was changed for the captures themselves.
