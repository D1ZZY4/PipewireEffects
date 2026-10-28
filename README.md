# PipewireEffects

A fast, modern PipeWire audio effects processor, with a GTK4 and libadwaita
interface, written in Rust.

It creates a virtual sink that applications can play into, runs the audio
through a chain of effects you build, and plays the result on the output device
you choose. That means an equaliser and a limiter for *any* application —
a browser, a game, a voice call — without any of them knowing about it.

```text
  applications  ──►  PipewireEffects Sink  ──►  effect chain  ──►  your device
                     (a real PipeWire sink)      EQ · limiter · gain
```

## What it does

Three effects, each with a full set of controls:

| Effect | What it is for | Parameters |
|---|---|---|
| **Parametric Equalizer** | Eight bands with per-band shape, frequency, gain and Q, plus a pre-amplifier | 41 |
| **Limiter** | A brick-wall peak limiter with make-up gain and a linked stereo detector | 4 |
| **Output Gain** | A simple gain stage with an optional mute | 2 |

Around them:

- A chain you can reorder, enable and bypass per effect, or bypass as a whole.
- Presets stored as readable JSON in `~/.local/share/pipewire-effects/presets`,
  with five built in.
- Live input and output level meters, and counters for dropped audio so a
  system that is quietly glitching is visible rather than merely audible.
- An output device chooser — the default is whatever the system is using, so
  plugging in headphones keeps working.

## Requirements

- PipeWire 0.3.49 or newer, running.
- GTK 4.14+ and libadwaita 1.5+. Development happens against GTK 4.24 and
  libadwaita 1.10, but the floor is kept at what the code actually needs, so it
  builds on current stable distributions.
- Rust 1.87+ (edition 2024).

On Arch:

```sh
sudo pacman -S pipewire gtk4 libadwaita rust
```

On Debian or Ubuntu the development packages are needed as well:

```sh
sudo apt install libpipewire-0.3-dev libgtk-4-dev libadwaita-1-dev rustc cargo
```

## Building and running

```sh
cargo build --release
./target/release/pipewire-effects
```

Useful flags:

```sh
pipewire-effects --list-effects          # what this build can do
pipewire-effects --list-devices          # what PipeWire can play on
pipewire-effects --device <node.name>    # force an output device
pipewire-effects --latency 40            # buffer latency, in milliseconds
pipewire-effects -vv                     # more logging
```

### Making applications use it

Route an application to the virtual sink. Easiest is to make it the default:

```sh
pactl set-default-sink pipewire-effects-sink
```

Or move one existing stream to it:

```sh
pactl list short sink-inputs             # find the stream index
pactl move-sink-input <index> pipewire-effects-sink
```

To undo it, set the default back to a real device:

```sh
pactl list short sinks                   # find the real sink's name
pactl set-default-sink alsa_output.pci-0000_00_1f.3.analog-stereo
```

Most desktop environments also let you pick the output device per application
in their own sound settings, where `PipewireEffects Sink` will now appear.

## Presets

Presets are JSON, and meant to be edited by hand. A complete one:

```json
{
  "schema_version": 1,
  "name": "Bass Boost",
  "description": "A shelf-and-bell low-end lift.",
  "author": "PipewireEffects",
  "output": {
    "bypassed": false,
    "effects": [
      {
        "kind": "parametric-eq",
        "enabled": true,
        "parameters": {
          "preamp": -4.0,
          "band1_type": 1,
          "band1_frequency": 60.0,
          "band1_gain": 8.0
        }
      },
      { "kind": "limiter", "parameters": { "threshold": -1.0, "makeup": 2.0 } }
    ]
  }
}
```

Effects are applied top to bottom. Parameter names match the sliders in the
interface; anything you leave out keeps its default, and anything the build
does not recognise is reported rather than rejected, so a preset from another
version still loads as far as it can.

Band shapes are indexed in this order:
`0 Peak`, `1 Low Shelf`, `2 High Shelf`, `3 High Pass`, `4 Low Pass`,
`5 Notch`, `6 Band Pass`, `7 All Pass`.

## How it is put together

Six crates, with dependencies running in one direction:

```text
core  ←  dsp  ←  presets        (presets only for testing)
  ↑
  └────  engine  ←  ui  ←  app
```

| Crate | Responsibility |
|---|---|
| `pipewire-effects-core` | The domain model: audio buffers, parameters, the effect trait, the chain, the lock-free ring buffer, meters |
| `pipewire-effects-dsp` | The effects and the DSP primitives they are built from |
| `pipewire-effects-presets` | The preset file format and storage |
| `pipewire-effects-engine` | PipeWire, the realtime path, and the counters that describe it |
| `pipewire-effects-ui` | The GTK4 interface |
| `pipewire-effects` | The binary that wires it together |

Two consequences are worth calling out.

**The interface does not know what an effect is.** Each effect declares a list
of parameters — a label, a type, a range, a unit — and the interface renders
whatever it finds: a range becomes a slider, a boolean becomes a switch, a list
becomes a drop-down. Adding an effect to `pipewire-effects-dsp` makes it fully
controllable with no changes to the interface at all.

**The audio thread never allocates, locks or frees.** Parameter changes travel
through atomics, structural edits through a lock-free queue, and effects that
are removed are handed to a recycle queue for another thread to dispose of.
[ARCHITECTURE.md](ARCHITECTURE.md) explains this in detail, along with why the
engine uses two streams rather than one filter node, and how the drift between
them is corrected.

## Tests

```sh
cargo test --workspace
```

186 tests, all of which run without a display or an audio device. They cover the
filter designs against their analytic frequency responses, the effects against
measured signal levels, the ring buffer against a concurrent producer and
consumer, and every preset that ships.

There is also a set of live tests that need a running PipeWire. They start the
real engine, feed it a tone with `pw-cat`, and check that the level comes out of
the chain where the effect settings say it should:

```sh
cargo test -p pipewire-effects-engine --test live_pipewire -- --ignored --nocapture
```

```text
captured 44800 frames, processed 42240 frames, buffer 14.7 ms,
input peak 0.5000, output peak 0.0500 (expected 0.0500), glitches 0
```

## Status

Early, and honest about it:

- Only the output pipeline exists. Processing the input (microphone) side is
  not implemented.
- Changing the output device restarts the audio engine. It takes well under a
  second, but it is a gap.
- Latency drift between the capture and playback streams is corrected by
  trimming the buffer when it grows. On a healthy system this never fires — the
  counter above shows zero — but a resampler would be the better answer.
- No packaging yet beyond a desktop file and an icon.

## Contributing

Bug reports and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md)
covers the setup, the conventions, and the three rules a change is not allowed
to break; participation is under the
[Code of Conduct](CODE_OF_CONDUCT.md). Security problems should go through
[SECURITY.md](SECURITY.md) rather than a public issue.

Changes are recorded in [CHANGELOG.md](CHANGELOG.md).

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE) for the full text, and
[CREDITS.md](CREDITS.md) for the projects this builds on.
