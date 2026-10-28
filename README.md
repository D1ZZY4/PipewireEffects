# PipeWire Effects

A PipeWire audio effects processor for the Linux desktop: Rust, native
GTK4/libadwaita, and PipeWire as the only audio backend.

It presents a virtual sink that applications can play into, runs the audio
through a chain of effects you build, and plays the result on the output device
you choose. Any application - a browser, a game, a call - can be processed
without knowing about it.

```text
  applications ──► PipeWire Effects Sink ──► effect chain ──► your device
                   (a virtual PipeWire sink)   up to 16 stages
```

## Effects

Thirteen effects, each with real processing and a generated control surface:

| Effect | What it does |
|---|---|
| **Equalizer** | Ten octave bands (31.5 Hz – 16 kHz) with an analytic response curve, shared bandwidth and output gain |
| **Filter** | Low pass, high pass, band pass, notch, all pass, peak and shelves, order 1–4 |
| **Compressor** | Feed-forward, peak or RMS detection, soft knee, attack/release, makeup, dry/wet |
| **Limiter** | Instantaneous-attack peak limiter with a hard ceiling and release |
| **Gate** | Noise gate with range, attack, hold and release |
| **De-esser** | Split or wideband sibilance reduction |
| **Auto gain** | Slow RMS normalisation towards a target level (not EBU R128) |
| **Bass enhancer** | Low-band harmonic generation so small speakers reproduce bass |
| **Crystalizer** | Four-band transient emphasis |
| **Stereo tools** | Width, balance and a Haas stereo base |
| **Gain** | A gain stage with mute |
| **Delay** | Independent left/right delay with feedback and ping-pong |
| **Reverberation** | Freeverb-style room with pre-delay, damping and width |

Around them:

- A chain you can reorder, enable per stage, or bypass as a whole.
- Presets stored as readable JSON, one built in, with save, delete, import and
  export. EasyEffects presets load too, converted on the way in.
- Live input and output meters (peak with an RMS marker).
- Honest counters: captured and processed frames, underruns, overruns and drift
  trims, plus a diagnostics list.
- An output device chooser.

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
- `compressor.detector`: `0` peak, `1` RMS.
- `deesser.mode`: `0` split, `1` wideband.

## How it is put together

```text
pwe-core  ◄── pwe-dsp  ◄── pwe-engine ◄── pwe-ui ◄── pipewire-effects
    ▲                          ▲            ▲
    └────────── pwe-presets ───┴────────────┘
```

| Crate | Responsibility |
|---|---|
| `pwe-core` | Buffers, parameter specifications and atomics, the effect contract, the chain, meters, realtime helpers |
| `pwe-dsp` | Biquad design, the thirteen effects, the effect registry |
| `pwe-presets` | Preset schema, validation, storage, built-ins |
| `pwe-engine` | PipeWire streams, virtual sink, realtime processor, device discovery, statistics |
| `pwe-ui` | GTK4/libadwaita interface |
| `pipewire-effects` | The binary |

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

203 tests, none of which need a display or an audio device. They cover biquad
designs against their analytic magnitude response, the effects against measured
signal levels and numerical bounds, the ring buffer and trimming policy with a
real producer and consumer, preset parsing, coercion and path-traversal
rejection.

Three live tests run the real engine against the running daemon, feed a tone
into a virtual sink with `pw-cat`, and check the level leaving the chain. The
processing test opens its own sink under a process-specific name, so the level it
measures is its own tone rather than whatever else the session has routed to the
shared sink:

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

- The virtual sink always passes audio at unity gain. Both internal streams opt
  out of the session manager's remembered stream volume and link target, so a
  value saved for the sink name in an earlier session cannot attenuate or
  retarget the chain.
- Only the output pipeline exists. Microphone processing is not implemented, so
  there is no virtual source.
- Changing the output device rebuilds both streams; audio stops for a fraction
  of a second.
- Drift between capture and playback is corrected by trimming the buffer, not by
  resampling.
- The limiter has no lookahead, so its instantaneous attack trades some
  transient distortion for a guaranteed ceiling.
- Auto gain is RMS-based, not EBU R128.
- The output device is chosen from `priority.session`; the
  `default.audio.sink` metadata is not read, so "follow the system default" is a
  heuristic.
- The plugin-based, neural and spectral effects of EasyEffects are not
  implemented. `docs/feature-matrix.md` lists every one with its status.

## Contributing

Bug reports and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md)
covers setup, conventions and the rules a change may not break. Participation is
under the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems should go
through [SECURITY.md](SECURITY.md) rather than a public issue.

Changes are recorded in [CHANGELOG.md](CHANGELOG.md).

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE) for the full text and
[CREDITS.md](CREDITS.md) for the projects this builds on.
