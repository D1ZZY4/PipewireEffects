# PipeWire Effects

A PipeWire audio effects processor for the Linux desktop: Rust, native
GTK4/libadwaita, and PipeWire as the only audio backend.

It presents a virtual sink that applications can play into, runs the audio
through a chain of effects you build, and plays the result on the output device
you choose. Any application - a browser, a game, a call - can be processed
without knowing about it.

It can also do the same in the other direction: it captures your microphone,
runs a second chain, and publishes the result as a virtual source that
applications can record from.

```text
  applications ──► PipeWire Effects Sink ──► chain ──► your output device
                   (a virtual PipeWire sink)  16 stages

  your microphone ──► chain ──► PipeWire Effects Source ──► applications
                      16 stages  (a virtual PipeWire source)
```

## Effects

Twenty effects, each with real processing and a generated control surface:

| Effect | What it does |
|---|---|
| **Equalizer** | Ten octave bands (31.5 Hz – 16 kHz) with an analytic response curve, shared bandwidth and output gain |
| **Filter** | Low pass, high pass, band pass, notch, all pass, peak and shelves, order 1–4 |
| **Mid/side equalizer** | The ten bands applied to the middle and the side separately, with mid and side trims |
| **Compressor** | Feed-forward, peak or RMS detection, soft knee, attack/release, makeup, dry/wet |
| **Limiter** | Instantaneous-attack peak limiter with a hard ceiling and release |
| **Maximizer** | Peak maximiser that lifts quiet material towards a ceiling, then clamps it |
| **Gate** | Noise gate with range, attack, hold and release |
| **Expander** | Downward expander with a soft knee, attack, release and makeup |
| **De-esser** | Split or wideband sibilance reduction |
| **Auto gain** | Slow RMS normalisation towards a target level (not EBU R128) |
| **Bass enhancer** | Low-band harmonic generation so small speakers reproduce bass |
| **Crystalizer** | Four-band transient emphasis |
| **Crusher** | Bit-depth and sample-rate reduction with a modulated hold |
| **Exciter** | Harmonic generation in a band above the scope frequency |
| **Stereo tools** | Width, balance and a Haas stereo base |
| **Crossfeed** | Leaks each channel into the other so headphones sound like speakers |
| **Crosstalk canceller** | Delayed reciprocal mix with a head-transfer correction and a phantom-centre mode |
| **Gain** | A gain stage with mute |
| **Delay** | Independent left/right delay with feedback and ping-pong |
| **Reverberation** | Freeverb-style room with pre-delay, damping and width |

Around them:

- Two pipelines: an **Output** chain and an **Input** (microphone) chain, each
  with its own device, meters and enable switch. The Effects page switches
  between them.
- A chain you can reorder, enable per stage, or bypass as a whole.
- Presets stored as readable JSON with `output` and `input` sections, one built
  in, with save, delete, import and export. EasyEffects presets load too, both
  sections, converted on the way in.
- Live input and output meters (peak with an RMS marker) for the pipeline you are
  editing.
- Honest counters: captured and processed frames, underruns, overruns and drift
  trims, plus a diagnostics list.
- Remembered settings: the device of each pipeline, the loaded preset and the
  bypass survive a restart.
- Processed audio follows the system default output device, and follows it when
  it changes; you can also pin a specific device. The microphone side follows the
  system default source the same way.
- A command line, and a *Start on login* switch that processes audio from
  sign-in.

## Requirements

- PipeWire 0.3.49 or newer, running.
- GTK 4.10+ and libadwaita 1.5+.
- Rust 1.92+ (edition 2024).

Arch:

```sh
sudo pacman -S pipewire gtk4 libadwaita rust
```

Debian or Ubuntu needs the development packages:

```sh
sudo apt install libpipewire-0.3-dev libgtk-4-dev libadwaita-1-dev rustc cargo
```

## Building and running

```sh
cargo build --release
./target/release/pipewire-effects
```

To install it into `$(PREFIX)` (default `/usr/local`), which also installs the
desktop entry, both icon sets and the built-in preset:

```sh
sudo make install
```

`packaging/` carries a `PKGBUILD` for Arch and a Flatpak manifest; the Flatpak
build still needs its generated `cargo-sources.json`.

## Command line

```text
pipewire-effects [options]

  -h, --help           Show the help and exit
  -V, --version        Show the version and exit
      --preset NAME    Load a preset on start
      --device NAME    Send the processed audio to this node
      --no-window      Start without a window; the interface opens on the next launch
      --list-presets   Print the available presets and exit
      --list-devices   Print the PipeWire devices and exit
```

`--no-window` is what the autostart entry uses: the processing starts at sign-in
and launching the application later opens the interface of the instance that is
already running.

## Making applications use it

Route an application to `pipewire-effects-sink`. The quickest way is to make it
the default output:

```sh
pactl set-default-sink pipewire-effects-sink
```

Or move a single stream:

```sh
pactl list short sink-inputs          # find the stream index
pactl move-sink-input <index> pipewire-effects-sink
```

Desktop environments also let you choose the output device per application, and
`PipeWire Effects Sink` appears there. To undo it, set the default back to a real
device:

```sh
pactl list short sinks
pactl set-default-sink <real sink name>
```

The devices tab shows the exact node name and has a button to copy it.

For the microphone side, turn on *Process microphone input* in the devices tab
and then pick `PipeWire Effects Source` as the input in the application that
records, or make it the default source:

```sh
pactl set-default-source pipewire-effects-source
```

## Settings

The device of each pipeline, the loaded preset, the bypass and the autostart
switch are remembered in `$XDG_CONFIG_HOME/pipewire-effects/settings.json`
(usually `~/.config/pipewire-effects/settings.json`). The file is written
atomically, and a damaged file falls back to the defaults with a message rather
than stopping the application.

## Presets

Presets are JSON and meant to be edited by hand. A complete one:

```json
{
  "schema_version": 1,
  "name": "Podcast",
  "output": {
    "bypassed": false,
    "effects": [
      {
        "kind": "filter",
        "params": { "mode": 1, "frequency": 80.0, "q": 0.70710678, "order": 2.0 }
      },
      {
        "kind": "equalizer",
        "params": { "band6_gain": 3.0, "band7_gain": 2.5 }
      },
      {
        "kind": "compressor",
        "params": { "threshold": -22.0, "ratio": 4.0, "makeup": 3.0 }
      },
      { "kind": "limiter", "params": { "limit": -1.0 } }
    ]
  }
}
```

Effects are applied top to bottom. Parameter identifiers are the ones each
effect declares in `crates/dsp` and are listed per effect in
[docs/feature-matrix.md](docs/feature-matrix.md); omitted parameters keep their
defaults. Unknown effects and
unknown parameters are reported when the preset loads rather than causing the
load to fail, and values are coerced to the parameter's type and range.

A preset can also carry an `input` section with the same shape, for the
microphone chain. It is written only when that chain is not empty, so files
without one keep loading.

Presets live in `$XDG_CONFIG_HOME/pipewire-effects/presets` (usually
`~/.config/pipewire-effects/presets`). A user preset shadows a built-in with the
same name, and deleting it brings the built-in back. The built-in preset is
compiled into the binary, so a fresh installation always has a usable chain.

Presets written by EasyEffects are recognised by their `plugins_order` layout
and converted on load: the effects and parameters with a close equivalent here
are mapped, band-based equalisers are folded onto the ten fixed bands, and
everything that could not be mapped is reported instead of being applied
silently.

Choice-valued parameters use these indices:

- `filter.mode`: `0` low pass, `1` high pass, `2` band pass, `3` notch,
  `4` all pass, `5` peak, `6` low shelf, `7` high shelf.
- `compressor.detector` and `expander.detector`: `0` peak, `1` RMS.
- `deesser.mode`: `0` split, `1` wideband.
- `crusher.mode`: `0` linear, `1` logarithmic.

## How it is put together

```text
pwe-core  ◄── pwe-dsp  ◄── pwe-engine ◄── pwe-ui ◄── pipewire-effects
    ▲                          ▲            ▲
    └────────── pwe-presets ───┴────────────┘
```

| Crate | Responsibility |
|---|---|
| `pwe-core` | Buffers, parameter specifications and atomics, the effect contract, the chain, meters, realtime helpers, the pipeline kinds |
| `pwe-dsp` | Biquad design, the twenty effects, the effect registry |
| `pwe-presets` | Preset schema, validation, storage, built-ins |
| `pwe-settings` | The settings document, its store and the autostart entry |
| `pwe-engine` | PipeWire streams, the virtual sink and virtual source, realtime processors, device discovery, statistics |
| `pwe-ui` | GTK4/libadwaita interface |
| `pipewire-effects` | The binary and its command line |

Two properties are worth calling out because later changes should preserve
them.

**The interface does not know what an effect is.** Each effect publishes a list
of parameter specifications - identifier, label, type, range, unit, grouping -
and the interface renders whatever it finds. Adding an effect to `pwe-dsp` makes
it fully controllable with no interface change.

**The audio thread never allocates, locks or frees.** Parameter changes travel
through atomics, structural edits through a lock-free queue with a fixed
capacity, and removed effects are handed to a recycle queue that a non-realtime
thread drains. [docs/architecture.md](docs/architecture.md) explains the rest,
including why two streams are used instead of PipeWire filter nodes, and how
drift between them is corrected.

## Tests

```sh
cargo test --workspace
```

311 tests, none of which need an audio device or a display; the two icon lookups
skip themselves when no display is available. They cover biquad designs against
their analytic magnitude response, the effects against measured signal levels and
numerical bounds, the ring buffer and trimming policy with a real producer and
consumer, the two meters against each other, preset parsing, coercion and
path-traversal rejection, and the gettext lookup.

Four live tests run the real engine against the running daemon, feed a tone
into a virtual sink with `pw-cat`, and check the level leaving the chain. The
processing test opens its own sink under a process-specific name, so the level it
measures is its own tone rather than whatever else the session has routed to the
shared sink; a fourth test starts the microphone pipeline and records from the
virtual source, which is how the input chain is verified:

```sh
cargo test -p pwe-engine --test live_pipewire -- --ignored --test-threads=1 --nocapture
```

```text
attempts 1; sampled 10 times; captured 16384 frames, processed 16384 frames, underruns 4096, overruns 0, trims 0, occupancy 0
tone measured at -20.00 dB with 20.00 dB of reduction; last read: input peak -20.00 dB, output peak -40.00 dB
```

That is a −20 dB gain stage measured end to end on real PipeWire, with the tone
reaching the chain at its own level.

There is also a window smoke test, which builds the real interface, adds an
effect, loads the built-in *Default* preset and exits with a status:

```sh
PIPEWIRE_EFFECTS_SMOKE_TEST=1 ./target/release/pipewire-effects
```

## Status

Working, and honest about the gaps:

- The virtual sink and the virtual source always pass audio at unity gain. All
  four internal streams opt out of the session manager's remembered stream volume
  and link target, so a value saved for either node name in an earlier session
  cannot attenuate or retarget the chain.
- Microphone processing is opt-in. The input engine, and therefore the capture of
  your microphone, starts only when *Process microphone input* is switched on.
- Changing a device rebuilds that pipeline's two streams; audio stops for a
  fraction of a second.
- Drift between capture and playback is corrected by trimming the buffer, not by
  resampling.
- The limiter has no lookahead, so its instantaneous attack trades some
  transient distortion for a guaranteed ceiling.
- Auto gain is RMS-based, not EBU R128.
- The underlying transport is two `pw_stream`s joined by a ring buffer, per
  pipeline, rather than one `pw_filter` node per effect, because `pipewire-rs`
  0.10 exposes no `pw_filter` binding. The cost is the buffer and its drift
  correction.
- A device you pick by hand is remembered, but a *change* of the system default
  is only followed while the application runs.
- The plugin-based, neural and spectral effects of EasyEffects are not
  implemented. `docs/feature-matrix.md` lists every one with its status.

## Translations

Every user-facing string is translated through gettext. The catalog template is
`po/pipewire-effects.pot`, `po/POTFILES.in` lists the files it is built from, and
`po/LINGUAS` lists the shipped languages. `scripts/update-po.sh` regenerates the
template and merges the `.po` files; `make install` compiles them into
`$(PREFIX)/share/locale`. See [CONTRIBUTING.md](CONTRIBUTING.md) for the details.
A build with no translation installed simply shows English.

## Contributing

Bug reports and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md)
covers setup, conventions and the rules a change may not break. Participation is
under the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems should go
through [SECURITY.md](SECURITY.md) rather than a public issue.

Changes are recorded in [CHANGELOG.md](CHANGELOG.md).

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE) for the full text and
[CREDITS.md](CREDITS.md) for the projects this builds on.
