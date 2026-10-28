# Architecture

This document describes the implementation that exists in this repository. It
is not a plan.

## 1. Crate graph

```text
pwe-core  ◄── pwe-dsp  ◄── pwe-engine ◄── pwe-ui ◄── pipewire-effects
    ▲                          ▲            ▲
    └────────── pwe-presets ───┴────────────┘
```

| Crate | Owns | Does not know about |
|---|---|---|
| `pwe-core` | Audio buffers, parameter specifications, the effect contract, the chain, meters, realtime-safe helpers | PipeWire, GTK, files |
| `pwe-dsp` | The thirteen effects, the biquad designs and the effect registry | Threads, devices, presets |
| `pwe-presets` | The preset schema, name validation and the preset directory | Which effects exist |
| `pwe-engine` | PipeWire streams, the virtual sink, the destination device, the realtime processor, counters | GTK, presets |
| `pwe-ui` | GTK4/libadwaita widgets, the chain model, the engine bridge | DSP internals |
| `pipewire-effects` | Process entry point | Everything else |

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

```text
  application ──► virtual sink (capture stream) ──► ring buffer ──► processor ──► playback stream ──► device
                  media.class = Audio/Sink          SPSC, f32        chain + meters     target.object
```

Two `pw_stream`s are used, joined into the same PipeWire node group
(`node.group` and `node.link-group` are both `pipewire-effects`):

- **Capture** (`pipewire-effects-sink`): direction `Input`, `media.class =
  Audio/Sink`, `node.virtual = true`, **not** auto-connected. Applications route
  into it, so it behaves like a sink without PipeWire trying to find a source for
  it.
- **Playback** (`pipewire-effects-output`): direction `Output`,
  `media.class = Stream/Output/Audio`, auto-connected with `target.object` (and
  `node.target`) set to the chosen sink node name.

Both streams set `state.restore-props = false` and `state.restore-target =
false`. A session manager remembers stream volumes and link targets by name, and
the virtual sink keeps a stable name, so a volume left behind by an earlier
session would silently scale everything entering the chain. These two nodes are
internal, they must pass audio at unity gain, and the device the playback stream
is linked to is chosen by this application, not restored from metadata.

A ring buffer between them absorbs scheduling jitter. The alternative - one
`pw_filter` node per effect, linked with `link-factory`, exactly as the reference
implementation does it - would remove the buffer and the drift correction, but
`pipewire-rs` 0.10 has no `pw_filter` binding. Hand-writing that binding would
concentrate unsafe code in the realtime callback, which is the least testable
place in the program. The two-stream design uses only the safe API. The cost is
documented in section 6.

### Why the DSP runs on the playback side

The playback stream is clocked by the hardware. The capture stream is clocked by
whatever is feeding the virtual sink. Running the chain on the hardware clock
gives the effects an even cadence.

### Choosing the destination

The engine binds the session manager's `default` metadata object and reads
`default.audio.sink`. It does this before it creates its own sink, so the value
it sees is a real hardware device rather than its own node, and it ignores any
value that names one of its two internal nodes. A destination chosen by hand in
the *Devices* page wins over the metadata; `default` in that control means
"follow the system default" again. When the metadata changes while the engine
runs, the two streams are rebuilt for the new device.

If neither an explicit choice nor the metadata names a usable sink, the engine
falls back to the highest `priority.session` among the discovered sinks that are
not itself and are not virtual.

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
| GTK main | Widgets, actions, a 50 ms refresh tick | Engine snapshot, meters, statistics |
| `pipewire-effects` | PipeWire main loop, `Loop::iterate` with a 20 ms timeout, command dispatch, effect disposal | Streams, device list, retired effects |
| PipeWire realtime | Capture submit, processor | Ring buffer, chain, meters |

The interface never calls a PipeWire API. It sends `EngineCommand` values over a
`std::sync::mpsc` channel and reads an `Arc<Shared>` snapshot: atomics for
meters, counters and state, a mutex for the device list and diagnostics. The
mutex is never taken by the audio thread.

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
  column (input meter, engine status, the effect rows, *Add effect*); the end
  side is the editor for the selected effect. Below 860 px the split view
  collapses and a header toggle reveals the chain over the editor.
- **Presets**: the preset library, with load, save, delete, import and export.
- **Devices**: engine status, the virtual sink name with a copy button, the
  output device chooser, statistics and diagnostics.
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

The window polls the engine every 50 ms. Nothing blocks: the snapshot is a short
mutex acquisition on a plain data structure.

### Icons

`data/io.github.D1ZZY4.PipewireEffects.gresource.xml` compiles the application
icon into the binary; `crates/ui/build.rs` runs `glib-compile-resources` and the
result is registered at startup, so the About dialog and the header bar find the
icon whether or not `data/icons` was installed. `crate::format` is the single
list of icon names, one per effect plus the chrome, and `format::missing_icons`
checks them against the live icon theme at startup and in a test. Every name is
part of the Adwaita theme: a name that only exists in an unrelated theme would be
drawn with a foreign glyph.

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

## 9. Error handling

- `CoreError`, `PresetError` and `EngineError` are explicit enums with `Display`
  and `Error` implementations. No error is discarded silently.
- PipeWire stream failures set the shared state to `Failed` and publish the
  message; the interface shows it in a banner.
- A missing output device does not stop the chain: it is reported as a warning
  and the destination is marked unavailable in the settings page.
- Preset problems carry the offending name or file path.

## 10. Verification

- `cargo test --workspace`: 211 tests, no audio device needed.
  Two of them need a display and skip themselves without one: the bundled icon
  lookup and the icon-theme check.
- Filter designs are checked against their analytic magnitude response, `Q` and
  gain sweeps, and a pole-stability sweep.
- Effects are checked against measured levels: 20 dB of gain, −6 dB of gain,
  limiter ceilings with 20 dB of drive, compressor ratios, gate range and hold,
  delay timing, reverb tail decay, de-esser selectivity, and numerical bounds
  under abusive settings.
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
  tone reaches the chain at and the level reduction of a −20 dB gain stage.
  Three tests, `#[ignore]`d so they do not run by default.
- `PIPEWIRE_EFFECTS_SMOKE_TEST=1` builds the real window, adds an effect, loads
  the *Default* preset and exits with a status. It requires a display and a
  running PipeWire.
- `PIPEWIRE_EFFECTS_UI_STATE=<state>` drives the real window into one state after
  it is mapped, so a screenshot can be taken of a specific screen. The states are
  `chain`, `editor`, `presets`, `devices`, `picker`, `picker-tone`, `about`,
  `save`, `banner`, `bypass` and `narrow`. This is the harness the interface
  review in section 12 used; it is not part of the product.

## 11. Performance

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

## 12. Interface review

The interface was reviewed by running the release binary under a separate X
server (`Xvfb`, the X11 backend) and capturing the window with ImageMagick's
`import` for each harness state listed in section 10, in both the dark and the
light appearance. What the review changed:

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

No PipeWire daemon was restarted, no device configuration was written, and no
system setting was changed for the review.
