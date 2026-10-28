# Architecture

This document explains how PipewireEffects is built and, more importantly, *why*
— including the places where a decision was a trade-off rather than a win.

---

## 1. Crate graph

```text
                    ┌──────────────────┐
                    │  pipewire-effects │  (binary)
                    └─────────┬─────────┘
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
      ┌──────────────┐ ┌─────────────┐ ┌──────────────┐
      │     -ui      │ │  -presets   │ │     -dsp     │
      └──────┬───────┘ └──────┬──────┘ └──────┬───────┘
             │                │               │
             ├────────────────┴───────────────┤
             ▼                                ▼
      ┌──────────────┐                ┌──────────────┐
      │   -engine    │───────────────▶│    -core     │
      └──────────────┘                └──────────────┘
```

Dependencies point one way and never loop. Each crate is the only place a
concern lives:

| Crate | Owns | Deliberately does *not* know about |
|---|---|---|
| `-core` | Buffers, formats, parameters, the `AudioEffect` trait, the chain, the ring buffer, meters | PipeWire, GTK, files |
| `-dsp` | Filter design and the effects | Threads, devices, presets |
| `-presets` | The file format and the directory presets live in | Which effects exist |
| `-engine` | PipeWire, the realtime path, counters | GTK, presets, which effects exist |
| `-ui` | GTK4 widgets and the action vocabulary | DSP, which effects exist |
| `pipewire-effects` | Wiring, and the command line | Everything else |

Two rules produced this shape.

**`-dsp` depends only on `-core`.** That is what makes every filter testable
against its analytic frequency response in microseconds, with no audio device in
sight. If a filter design could reach for a device or a logger, the tests would
eventually start doing so too.

**Nothing depends on a specific effect.** The registry in `-dsp` is passed *into*
`-engine` and `-ui` as a parameter. Neither crate can name an effect, which is
why the interface can control an effect that did not exist when it was written.

---

## 2. The audio path

```text
   application (pw-cat, Firefox, a game …)
        │  output ports
        ▼
   ┌─────────────────────────────────────────┐
   │ capture stream                          │
   │   media.class = Audio/Sink              │   ← looks like a real sink
   │   process() on the graph's RT thread    │      to every other client
   └───────────────────┬─────────────────────┘
                       │  CaptureSource::capture()
                       │  · input meters
                       ▼
   ┌─────────────────────────────────────────┐
   │ lock-free ring buffer (SPSC)            │   ← absorbs scheduling jitter
   └───────────────────┬─────────────────────┘
                       │  RtProcessor::process()
                       │  · drain chain commands
                       │  · trim drift
                       │  · run the effect chain
                       │  · sanitise, output meters
                       ▼
   ┌─────────────────────────────────────────┐
   │ playback stream                         │
   │   media.class = Stream/Output/Audio     │
   │   target.object = your device           │
   └───────────────────┬─────────────────────┘
                       ▼
                  output device
```

Both streams join the same PipeWire **node group** (`node.group` and
`node.link-group`), which is what lets the server schedule the two halves as a
unit rather than letting them wander into different graph cycles.

The capture stream is deliberately **not** auto-connected. It is a sink:
PipeWire must not go looking for a source to feed it. Its input ports exist for
applications that choose to route there.

### Why the DSP runs on the playback side

The playback stream is driven by the hardware clock, which is the stable one.
The capture stream is driven by whatever is playing into the virtual sink, which
is far less predictable. Running the chain on the stable side means the effect
chain sees an even cadence regardless of what the client application is doing.

---

## 3. Realtime rules

A dropout is not a crash: it is a few milliseconds missing from the middle of a
song. That makes realtime violations easy to ship and hard to notice, so the
rules are structural rather than advisory.

### Rule 1 — the audio thread never allocates, locks or frees

| What changes | How it reaches the audio thread | Cost per block |
|---|---|---|
| Parameter values (a slider moves) | `AtomicU32` holding `f32` bits, read once per block | a relaxed load |
| Chain structure (add / remove / reorder) | `crossbeam_queue::ArrayQueue<ChainCommand>`, drained at the top of the block | a lock-free pop |
| Removing an effect | The effect is handed to a *recycle* queue, not dropped | a `Box` move |

Removal is the interesting one. Dropping a `Box<dyn AudioEffect>` deallocates,
and the allocator may take a lock. So `ChainCommand::apply` takes a `retire`
callback that never drops anything, and `RtProcessor` pushes retired effects
onto a queue the engine's own non-realtime thread drains between loop
iterations. That is also why `ChainCommand::apply` is written as a free-standing
method over `&mut EffectChain` plus a callback, rather than a method on the
chain.

### Rule 2 — the interface only ever publishes intent

Widgets emit a `UiAction`. One function, `State::handle`, performs the mutation
through the domain objects and rebuilds the views that changed. Reads happen on
a separate 20 Hz tick that never writes.

Keeping writes on the action path and reads on the tick path is what stops a
redraw from racing an edit. It also means the entire interface is a single-
threaded, ordinary Rust program: no `unsafe`, no locks, and the only shared
mutable state is a set of atomic cells.

### Rule 3 — problems are counted, not hidden

`StreamStats` carries underruns, overruns, latency trims, dropped retired
effects and the current buffer occupancy. The interface shows them.

This is not decoration. During development of the loopback the failure mode was
never a crash or an error message — it was audio that sounded *almost* right.
The counters are what made the difference between "seems to work" and the
measured result in the README.

---

## 4. Why two streams and not one `pw_filter`

**This is the most significant trade-off in the project, so it is worth stating
plainly.**

PipeWire has a node type built for exactly this job: `pw_filter`. A filter node
has both input and output ports in a single node, so a chain of them can be wired
together with explicit links and no buffer anywhere in the path:

```text
  virtual sink (a real node, created through the factory system)
      │  monitor ports
      ▼
  [filter] → [filter] → [spectrum] → [level meter]
      │
      ▼
  physical device          (linked through the link factory)
```

That is the better topology. There is no buffer between the halves, so there is
no jitter to absorb and no drift to correct. It also integrates more cleanly with
the session manager, because the virtual device is a real node rather than a
stream that happens to claim `Audio/Sink`.

**`pipewire-rs` does not bind `pw_filter`.** As of 0.10 there is no `filter`
module; the safe API covers `pw_stream`, proxies, the registry and the
factories, and nothing else. Writing the binding by hand means a substantial
amount of `unsafe` around callbacks that run on the realtime thread — the part
of the codebase where a mistake is least survivable and least testable.

The alternative taken here is the one PipeWire's own `pw-loopback` tool uses: two
`pw_stream`s, one acting as the sink, one playing out. It uses only the safe API
and it works. The cost is a ring buffer and the drift correction below.

**What would change my mind:** if `pipewire-rs` gains `pw_filter` bindings, the
right move is to replace the two streams with one filter node and delete
`crates/core/src/rt.rs` entirely. The engine is structured so that this is
contained: nothing outside `-engine` knows how many streams there are.

---

## 5. Drift correction

The capture and playback streams are driven by clocks that are close but not
identical, and they are scheduled independently. Over a long session the ring
buffer would either drain (underruns) or fill (unbounded latency).

The policy splits by ownership, because a single-producer/single-consumer ring
buffer only lets each side touch its own index:

| Situation | Who fixes it | How | Counter |
|---|---|---|---|
| Buffer empty | Consumer (playback) | Pad with **silence**, not a repeated sample | `underrun_samples` |
| Buffer full | Producer (capture) | Drop the **newest** samples | `overrun_samples` |
| Buffer too full | Consumer (playback) | Discard the **oldest** samples, back to target | `trim_events` |

Trimming happens *before* the read, on the consumer side, because that is the
only side that may move the read cursor.

Padding with silence rather than repeating a sample is a deliberate choice: a
short gap is much less noticeable than a click, and a click is what a repeated
sample sounds like at a buffer boundary.

Debug builds help here too: subnormal arithmetic is dramatically slower on
mainstream CPUs, and a filter fed silence decays into subnormals. Filter state is
therefore sanitised once per block — which also clears any `NaN` that arrived
from upstream, bounding the damage to a single block instead of silencing the
stream permanently.

---

## 6. The parameter model

An effect declares **metadata**, not an interface:

```rust
ParamSpec::float("band4_gain", "Gain", -24.0, 24.0, 0.0, 0.1, ParamUnit::Decibel)
ParamSpec::float_log("band4_frequency", "Frequency", 20.0, 20_000.0, 1_000.0, ParamUnit::Hertz)
ParamSpec::boolean("band4_enabled", "Band 4", true)
ParamSpec::enumerate("band4_type", "Shape", &TYPE_LABELS, 0)
```

and keeps its live values in atomics:

```rust
let parameters = ParamSet::new(specs);
```

Everything else follows from that split:

- **The interface is generic.** `-ui` renders whatever specs it finds. It has
  never seen a parametric equaliser and does not need to.
- **Slider changes need no message passing.** A control writes the atomic cell;
  the DSP reads it at the top of the next block. There is no queue, no command
  and no rebuild, which is why dragging a slider cannot glitch the audio.
- **Grouping is free.** The equaliser's 41 parameters would be an unusable flat
  list, but the identifiers already carry the structure — `band4_gain`,
  `band4_frequency` — so the editor splits on the first underscore and uses the
  prefix as a heading. No extra metadata was needed.
- **Logarithmic sliders are a flag.** A slider works in `0.0..=1.0` normalised
  space and maps through `ParamKind::denormalise`, so the `logarithmic` flag
  makes a frequency slider feel linear in pitch rather than in hertz. The same
  code path serves both, because the mapping is a property of the parameter.

Values are stored as `f32` bits in an `AtomicU32`, with booleans as `0.0`/`1.0`
and enumeration indices as their index. The specification is what restores the
richer type. That uniformity is why one `AtomicParam` type can back every
control in the interface.

### The chain, and staying in step

The interface and the audio thread each hold a copy of the chain's *shape*, and
both apply the same `ChainCommand`s in the same order. They stay consistent
without any shared lock because the operations are deterministic and the queue
preserves order.

The subtle part is that the instance sent to the audio thread and the parameters
kept in the interface model must be the **same instance's** cells —
`ParamSet::clone` shares the `Arc`, so cloning *before* moving the instance is
what connects a slider to the DSP. `crates/core/src/control.rs` has a test named
for exactly that property, because getting it wrong produces an interface that
looks perfect and affects nothing.

---

## 7. Threads

| Thread | Runs | Touches |
|---|---|---|
| GTK main | Widgets, actions, the 20 Hz tick | `ChainController`, meters, stats |
| `pipewire-effects` | PipeWire main loop (`Loop::iterate` with a 50 ms timeout) | Streams, and disposal of retired effects |
| PipeWire RT | `CaptureSource::capture`, `RtProcessor::process` | Ring buffer, chain, meters, stats |
| `pw-dump`-style short-lived | `list_devices` | Nothing shared |

The engine drives its own loop with a timeout rather than calling
`MainLoop::run`, so a stop request is honoured within 50 ms without needing to
send a raw pointer to the main loop across threads. Disposal of retired effects
happens on that thread between iterations: it is not realtime, and it is always
running.

`list_devices` opens a throwaway connection for tens of milliseconds rather than
adding registry handling to the running engine. A device list can be refreshed
without disturbing playback, and the audio thread never sees the code.

---

## 8. Presets

The file format is a **compatibility surface**, so it is its own crate with its
own schema version, its own tests, and no dependency on which effects exist.

```json
{
  "schema_version": 1,
  "name": "Bass Boost",
  "output": {
    "bypassed": false,
    "effects": [
      { "kind": "parametric-eq", "parameters": { "band1_gain": 8.0 } }
    ]
  }
}
```

Decisions and their reasons:

- **Unknown things are reported, not rejected.** A preset from a newer build
  should still load as far as it can, so `apply_recipe` returns a report of
  skipped effects and unrecognised parameter names rather than failing.
- **Values are coerced to the target parameter's type.** A preset that writes
  `true` for a float parameter yields `1.0`. Hand-edited presets stay usable.
- **Parameters are stored in a `BTreeMap`.** Saved presets come out
  alphabetically ordered, which diffs cleanly and reviews easily.
- **A user preset shadows a built-in of the same name, and is still deletable.**
  Deleting it falls back to the built-in. Reporting it as built-in would have
  made it impossible to remove the override from the interface.
- **Malformed user presets are skipped, not fatal.** One bad file must not empty
  the preset browser.

The `output` section is separate from `name` and friends so that an `input`
section can be added under `schema_version: 2` without disturbing anything that
works today.

---

## 9. Verification

The interesting question is not "does it compile" but "does it sound right",
and the answer is a layered set of measurements rather than an opinion.

**Filter designs are checked against analytic responses.** A peaking filter at
+6 dB must measure +6 dB at its centre frequency and return to unity at 20 Hz
and 18 kHz. Every design is swept across frequency, Q and gain and required to
produce a stable section — poles inside the unit circle.

**Effects are checked against measured signal levels.** A −6 dB gain must halve
the amplitude; a fresh equaliser must be bit-exact transparent; a boost at
1 kHz must not leak 0.3 dB into 100 Hz; a limiter must never emit a sample above
its ceiling, including with 20 dB of make-up gain.

**The ring buffer is checked under concurrency.** A producer and a consumer on
real threads move 20,000 blocks and assert every sample arrives in order.

**The whole thing is checked against a live daemon.** The live tests start the
real engine, wait for the virtual sink to appear as an audio device, feed it a
tone with `pw-cat`, and read the engine's own output meters — which sit after the
chain and before the device:

```text
captured 44800 frames, processed 42240 frames, buffer 14.7 ms,
input peak 0.5000, output peak 0.0500 (expected 0.0500), glitches 0
```

That single line covers the whole path. The audio arrived from a real client
(44,800 frames captured), the chain ran, the level came out at exactly the value
the gain setting implies (0.05 = 0.5 × 10^(−20/20)), and nothing was dropped or
invented. If the streams were not linked, or the chain was not running, or the
meters were reading the wrong buffer, the numbers would not line up.

The tests are layered deliberately: the DSP tests run in milliseconds and catch
most mistakes; the live tests take seconds and need a device, so they are
`#[ignore]`d rather than being the first thing a contributor runs into.

---

## 10. Known gaps

- **No input pipeline.** Only output effects exist. The preset schema has room
  for an `input` section, and the engine's structures are symmetric, but the
  virtual source side is not implemented.
- **Device changes restart the engine.** Changing the target means reconnecting
  a stream, which PipeWire cannot do in place. Under a second, but visible.
- **Drift is trimmed, not resampled.** A resampler would preserve the audio
  instead of discarding a sliver of it. On a healthy system the trim counter
  stays at zero, so the simpler answer is currently adequate.
- **No lookahead in the limiter.** Instantaneous attack guarantees the ceiling,
  at the cost of some distortion on transients. A lookahead variant would need
  latency reporting, which `AudioEffect::latency_frames` already exists to
  carry.
- **No packaging.** A desktop file and an icon exist; Flatpak and distribution
  packaging do not.
