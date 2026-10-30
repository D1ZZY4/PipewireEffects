# Credits

PipeWire Effects exists because other people did hard work first. Thank you to
all of them.

## Inspiration

**[EasyEffects](https://github.com/wwmm/easyeffects)** by Wellington Wallace and
contributors, GPL-3.0-or-later.

The EasyEffects source is available in `references/` and was studied to
understand how a PipeWire effects processor should behave: presenting itself to
the system as an ordinary audio device, letting the user own the effect order,
and exposing real meters rather than decorative ones.

No source code, resource or asset was copied, with two documented exceptions: the
cross-talk canceller in `crates/dsp/src/effects/crosstalk.rs` is a port of
EasyEffects' own native implementation of that effect, by Antti S. Lankila, and
carries its delay-line layout, correction-filter constants and phantom-centre
mode; and the voice suppressor's symbolic icon
(`data/icons/hicolor/symbolic/apps/pipewire-effects-voice_suppressor-symbolic.svg`)
reuses the path data of the Adwaita icon theme's `action-unavailable-symbolic`.
Both projects' licences are compatible with this one. Everything else is an
independent Rust implementation and an original icon; the debt is one of ideas.
`docs/reference-audit.md` records exactly which files were consulted and what was
taken from them.

## Standing on

### PipeWire

**[PipeWire](https://pipewire.org/)** - the audio server this is built on. MIT.

**[pipewire-rs](https://gitlab.freedesktop.org/pipewire/pipewire-rs)** plus
`libspa` and their `-sys` crates - every interaction with the server goes
through them. MIT.

The two-stream loopback in `crates/engine` follows the approach used by
`pw-loopback`, one of PipeWire's own tools.

### The desktop stack

**[GTK 4](https://gtk.org/)** - the toolkit the interface is built from.
LGPL-2.1-or-later.

**[libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/)** - the GNOME
interface patterns expressed as widgets. LGPL-2.1-or-later.

**[gtk4-rs](https://github.com/gtk-rs/gtk4-rs)** and
**[libadwaita-rs](https://gitlab.gnome.org/World/Rust/libadwaita-rs)**, with
`glib`, `gio` and `cairo-rs` beneath them. MIT.

The **Adwaita icon theme** by the GNOME Project, LGPL-3.0-or-later or
CC-BY-SA-3.0 - the palette of symbolic icon names the interface draws from, and
the path data behind one of the icons the application ships (see above).

### DSP references

**Robert Bristow-Johnson's ["Audio EQ Cookbook"](https://www.w3.org/TR/audio-eq-cookbook/)**
- the formulas behind every biquad in `crates/dsp/src/biquad.rs`. The tests that
hold each design to its analytic frequency response exist because the maths is
written down properly.

**Freeverb** by Jezar at Dreampoint - the comb and all-pass topology behind
`crates/dsp/src/effects/reverb.rs`, reimplemented with rate-dependent tuning.

### Direct dependencies

| Crate | Used for | Licence |
|---|---|---|
| [`pipewire`](https://crates.io/crates/pipewire) | PipeWire client, streams, SPA format negotiation | MIT |
| [`crossbeam-queue`](https://crates.io/crates/crossbeam-queue) | The lock-free queue between the control and audio threads | MIT / Apache-2.0 |
| [`rtrb`](https://crates.io/crates/rtrb) | The realtime-safe single-producer/single-consumer ring buffer | MIT / Apache-2.0 |
| [`serde`](https://crates.io/crates/serde) + [`serde_json`](https://crates.io/crates/serde_json) | The preset file format | MIT / Apache-2.0 |
| [`gtk4`](https://crates.io/crates/gtk4) | The interface | MIT |
| [`libadwaita`](https://crates.io/crates/libadwaita) | GNOME widgets and patterns | MIT |

And **[Rust](https://www.rust-lang.org/)**, which makes realtime audio code
possible to reason about: the ownership rules are what let the audio thread stay
lock-free without becoming a minefield.

---

If something here has been used without proper credit, that is a mistake rather
than an intention. Please open an issue.
