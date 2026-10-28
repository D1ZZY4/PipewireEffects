# Reference audit

## 1. What the reference is

| Property | Value |
|---|---|
| Project | EasyEffects |
| Location | `references/` (a separate Git checkout, left untouched) |
| Revision | `7e314dc53`, `git describe` → `v8.3.0-242-g7e314dc53`, branch `master` |
| Declared version | `8.3.0` (`references/CMakeLists.txt`, `project(easyeffects VERSION 8.3.0)`) |
| Language | C++20 and C17 |
| UI | Qt 6, QML, KDE Frameworks 6, Kirigami |
| Build | CMake 3.28+ |
| Licence | GPL-3.0-or-later (`references/LICENSE`) |

The checkout also contains branches for the project's history, including
`gtk4-legacy`, `gtkmm-legacy`, `pipewire-gstreamer-legacy`,
`pulseaudio-legacy`, `python` and `rust`. Only `master` was read.

## 2. Coverage of this audit

This audit was targeted at the subsystems needed to design an independent Rust
implementation. It was **not** a line-by-line reading of every file in the
checkout; the checkout is large (C++ sources, 30+ translation catalogues, help
pages, Flatpak manifests, images). Files were read in full where they were used
as evidence; the table below records exactly which ones.

| Area | Files inspected | What was established |
|---|---|---|
| Project identity | `README.md`, `CMakeLists.txt`, `.github/`, `debian/`, `PKGBUILD` | Version, licence, effect inventory, dependency list, build requirements |
| Effect inventory | `README.md`, `src/effects_base.cpp`, `src/stream_output_effects.hpp` | The 33 plugin constructors and the attribute set used to build the pipeline |
| Plugin contract | `src/plugin_base.hpp` | Every effect is a `pw_filter` node with two input and two output ports plus optional probe ports |
| PipeWire lifecycle | `src/pw_manager.hpp`, `src/pw_manager.cpp` | One `pw_thread_loop`, one `pw_core`, one registry, metadata and node/link/module/client/device managers |
| Virtual devices | `src/pw_node_manager.cpp` (`load_virtual_devices`) | Sink and source are created with `pw_core_create_object(core, "adapter", ...)` and `factory.name = support.null-audio-sink` |
| Naming | `src/tags_pipewire.hpp` | `easyeffects_sink`, `easyeffects_source`, media classes |
| Presets | `src/presets_manager.cpp` (partially), `src/presets_manager.hpp` | JSON layout, `plugins_order`, blocklist, user directories, directory watcher |
| Documentation | `src/contents/docs/plugins/*.md` (directory listed, limiter and a few pages read), `src/contents/docs/user_interface/*.md` | Documented behaviour and parameter intent |

Not inspected in depth: the individual DSP implementations of the effects
(`*.cpp` bodies), the QML view tree, kcfg schemas, translation catalogues, and
packaging manifests beyond their existence. No claim in this repository depends
on the unread material.

## 3. Architecture of the reference

```text
pw::Manager (pw_thread_loop)
 ├── registry, metadata, node / link / module / client / device managers
 ├── virtual sink  = adapter node, factory.name = support.null-audio-sink
 ├── virtual source= adapter node, factory.name = support.null-audio-sink
 └── StreamOutputEffects / StreamInputEffects
      └── PluginBase × N   (one pw_filter node per enabled effect)
           linked with the link-factory between sink, filters and device
```

Facts that shaped the Rust design:

1. **The virtual device is a real node**, created through the factory system
   with `support.null-audio-sink`, `media.class = Audio/Sink` (or
   `Audio/Source`), `node.virtual = true`, `monitor.passthrough = true` and
   `priority.session = 0`.
2. **Each effect is a filter node**, not a software stage inside one process.
   The chain is a set of `pw_filter`s linked with `link-factory` objects, so the
   audio never leaves the PipeWire graph.
3. **The effects are third-party plugins.** Most of the inventory is a wrapper
   around LSP, Calf, ZamAudio, MDA, SpeexDSP, SoundTouch, RNNoise,
   DeepFilterNet, HushMic, x42 or Rakarrack. Only a minority are implemented in
   the repository.
4. **Presets are JSON with a flat plugin map**: `output.plugins_order` plus
   `output.<plugin>#<instance>` objects, and a `blocklist` of applications.
5. **Settings are KConfig-based** (`src/contents/kcfg`), including buffer size,
   samplerate and the last used preset.

## 4. Dependency consequences

The reference's dependency list (from `CMakeLists.txt` and `README.md`) is
substantial: libpipewire, lilv, libebur128, fftw3/fftw3f, speexdsp,
nlohmann_json, libmysofa, GSL, libbs2b, libsndfile, rnnoise, soundtouch,
webrtc-audio-processing, zita-convolver, TBB, Qt 6, KF6.

For this project the following were judged out of scope, with reasons recorded
in `docs/feature-matrix.md`:

- LV2/LADSPA plugin hosting (would require `lilv` and a plugin graph; the
  project's goal is a self-contained Rust application).
- FFT convolution (needs an FFT dependency and impulse-response management).
- Neural noise suppression (needs ONNX/RNNoise models and a large runtime).
- Pitch shifting and autotune (needs a phase vocoder or SoundTouch-like
  library).

## 5. Licence position

EasyEffects is GPL-3.0-or-later. This project is also GPL-3.0-or-later
(`LICENSE`, `Cargo.toml`), so the licences are compatible.

**No EasyEffects source, resource or asset was copied.** The Rust implementation
was written from scratch against the behavioural findings above. The biquad
formulas come from Robert Bristow-Johnson's Audio EQ Cookbook, which is a public
specification; the reverb topology follows the published Freeverb design. Both
are credited in `CREDITS.md`.

The reference directory was not modified at any point.

## 6. Findings that were deliberately not adopted

| Reference behaviour | Decision here |
|---|---|
| One `pw_filter` per effect | `pipewire-rs` 0.10 exposes no `pw_filter` binding; two `pw_stream`s with a ring buffer are used instead. Documented in `docs/architecture.md`. |
| Third-party plugin wrappers | Replaced with native Rust DSP for a self-contained binary; the plugin-based effects are listed as unimplemented rather than faked. |
| 30-band equalizer with per-band type/Q and channel splitting | A 10-band octave equalizer with a shared bandwidth and output gain. Reduction recorded in the matrix. |
| EBU R128 loudness (libebur128) | RMS-based auto gain. The measurement is not R128 and is labelled as such. |
| KConfig settings | Plain JSON presets plus runtime-only engine settings. |
| Metadata-driven default device | `priority.session` heuristic, documented as a limitation. |