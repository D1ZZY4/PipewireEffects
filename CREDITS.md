# Credits

PipewireEffects exists because a lot of other people did a lot of hard work
first. Thank you to all of them.

## Inspiration

**[EasyEffects](https://github.com/wwmm/easyeffects)** by Wellington Wallace and
contributors, licensed GPL-3.0-or-later.

EasyEffects showed that this kind of application is worth building and how it
ought to behave: that an effects processor should present itself to the rest of
the system as an ordinary audio device, that the effect chain belongs to the
user rather than to the application, and that a GraphQL-free, no-nonsense
interface with live meters is more useful than a clever one. Working out *how*
it solved those problems is what made the design here possible at all.

No code was copied. PipewireEffects is an independent implementation in Rust,
and shares no source with EasyEffects. The debt is one of ideas, and it is
genuinely owed.

## Standing on

### PipeWire

**[PipeWire](https://pipewire.org/)** — the audio server this is built on, and
the reason any of this is possible on Linux at all. MIT licensed.

**[pipewire-rs](https://gitlab.freedesktop.org/pipewire/pipewire-rs)**, plus
`libspa` and their `-sys` crates — the Rust bindings used for every interaction
with the server. MIT licensed.

The two-stream loopback in `crates/engine` follows the approach taken by
`pw-loopback`, one of PipeWire's own command-line tools.

### The desktop stack

**[GTK 4](https://gtk.org/)** — the toolkit the interface is built from, and a
decade and a half of accumulated wisdom about how a desktop application should
behave. LGPL-2.1-or-later.

**[libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/)** — the GNOME
human interface guidelines, expressed as widgets. LGPL-2.1-or-later.

**[gtk4-rs](https://github.com/gtk-rs/gtk4-rs)** and
**[libadwaita-rs](https://gitlab.gnome.org/World/Rust/libadwaita-rs)** — the
bindings that let the interface be written in Rust, plus `glib` and `gio`
beneath them. MIT licensed.

### Filter design

**Robert Bristow-Johnson's ["Audio EQ Cookbook"](https://www.w3.org/TR/audio-eq-cookbook/)**
— the formulas behind every biquad in `crates/dsp/src/biquad.rs`. A peaking
filter, a shelf and a high-pass are all defined there in terms of the parameters
a musician actually understands, and the derivation is the reason those
definitions are trustable. The tests that hold each design to its analytic
frequency response are possible because the maths is written down properly.

### Everything else

The crates this depends on, each of which solved a problem so it did not have to
be solved again:

| Crate | Used for | Licence |
|---|---|---|
| [`crossbeam-queue`](https://github.com/crossbeam-rs/crossbeam) | The lock-free queue between the interface and the audio thread | MIT / Apache-2.0 |
| [`serde`](https://serde.rs/) + `serde_json` | The preset file format | MIT / Apache-2.0 |
| [`thiserror`](https://github.com/dtolnay/thiserror) | Error types | MIT / Apache-2.0 |
| [`tracing`](https://tokio.rs/) + `tracing-subscriber` | Logging | MIT |
| [`directories`](https://github.com/dirs-dev/directories-rs) | Finding the right place to keep presets | MIT / Apache-2.0 |
| [`clap`](https://github.com/clap-rs/clap) | The command line | MIT / Apache-2.0 |

And **[Rust](https://www.rust-lang.org/)** itself, which makes it possible to
write realtime audio code that is also safe to reason about — the ownership
rules are what let the audio thread go lock-free without becoming a minefield.

---

If something here has been used without proper credit, that is a mistake rather
than an intention. Please open an issue.
