# Feature matrix

## 0. Scope classification

Every candidate below is classified as one of:

- **REQUIRED** - part of this application's stated purpose: process application
  audio through a user-built chain on a virtual PipeWire sink, with real DSP, a
  native interface, persistence and diagnostics. Must be implemented and
  verified.
- **PRESERVE** - behaviour or infrastructure that had to survive the rewrite.
- **EXCLUDED** - deliberately outside this application's scope, with the reason
  recorded. A product-scope decision, not a claim of impossibility.
- **BLOCKED** - intended, but cannot be completed in this environment.

This repository set out to be a self-contained Rust application, so the
classification is:

| Class | Items |
|---|---|
| REQUIRED | Thirteen effects; the virtual sink; the output pipeline; the destination device; per-effect enable, ordering and global bypass; presets (built-in, user, import, export, EasyEffects conversion); live meters; diagnostics; the GTK4/libadwaita interface; the desktop entry and icon |
| PRESERVE | Preset JSON schema v1 and its reader; the easyeffects conversion vocabulary and its warning list; the existing module boundaries and the realtime rules; the repository identity, licence and documentation |
| EXCLUDED | LV2/LADSPA hosting, the convolver, the spectrum analyser, pitch shift, crossfeed, crusher, expander, exciter, mid/side equalizer, multiband compressor and gate, maximizer, bass loudness, autotune, crosstalk canceller, voice suppressor, echo canceller, speech processor, loudness metering. Reasons: each needs a third-party plugin, an FFT/phase-vocoder library or a neural model, which conflicts with the self-contained-binary goal; the per-row reason says which |
| BLOCKED | Deep noise removal and RNNoise noise reduction (require ONNX/RNNoise models and a runtime that cannot be verified here); the community preset browser (requires network access). The input (microphone) pipeline, preset autoload per application, the blocklist, the last-used-preset store, translations, autostart, the command socket and distribution packaging are **not started** and are recorded as unfinished, not as excluded |

The application is therefore complete for the output-pipeline scope described in
`README.md`, and is **not** at full EasyEffects parity. See
`docs/architecture.md` for what is verified.

Status values:

- **Implemented** - behaves as described and is covered by tests.
- **Partial** - the capability exists with a documented reduction in scope or
  fidelity.
- **Not implemented** - absent; the reason is recorded.
- **Not applicable** - a reference-specific mechanism with no equivalent here.

Reference evidence refers to files under `references/`.

## 1. Effects

| Effect | Reference evidence | Reference behaviour | Module here | Status | Notes and tests |
|---|---|---|---|---|---|
| Equalizer | `src/equalizer.cpp`, `src/equalizer_apo.cpp`, `docs/plugins/equalizer.md` | 30 bands, per-band type (peak/shelf/pass/notch), frequency, gain, Q; APO import; per-channel split | `crates/dsp/src/effects/equalizer.rs` | Partial | 10 fixed ISO octave bands (31.5 Hz – 16 kHz), shared bandwidth, output gain, analytic response curve. Tested: flat transparency, band boost accuracy against the analytic response, leakage, output gain. |
| Filter | `src/filter.cpp`, `docs/plugins/filter.md` | Low/high/band-pass and band-reject | `crates/dsp/src/effects/filter.rs` | Implemented | Eight shapes (the four above plus notch, all-pass, peak, shelf), order 1–4, frequency, Q, gain. Tested against measured pass/stop bands, slope growth with order, notch rejection, stability. |
| Compressor | `src/compressor.cpp`, `docs/plugins/compressor.md` | Feed-forward compressor, peak/RMS detector, ratio, knee, attack/release, makeup | `crates/dsp/src/effects/compressor.rs` | Partial | Threshold, detector, ratio, knee, attack, release, makeup, mix; stereo-linked detector. No sidechain input, no per-band controls. Tested: below-threshold transparency, ratio accuracy, makeup, dry/wet. |
| Limiter | `src/limiter.cpp`, `docs/plugins/limiter.md` | Lookahead limiter (LADSPA) | `crates/dsp/src/effects/limiter.rs` | Partial | Instantaneous-attack peak limiter with ceiling and release. No lookahead, therefore no reported latency and some transient distortion. Tested: ceiling never exceeded with +20 dB input, unity for quiet material, gain recovery. |
| Gate | `src/gate.cpp`, `docs/plugins/gate.md` | Noise gate with threshold, range, attack, hold, release | `crates/dsp/src/effects/gate.rs` | Partial | Same parameter set plus mix. No sidechain or hysteresis controls. Tested: open on loud material, range attenuation, hold delay. |
| De-esser | `src/deesser.cpp`, `docs/plugins/deesser.md` | Sibilance reduction, split or wideband | `crates/dsp/src/effects/deesser.rs` | Implemented | Band detector, threshold, ratio, split/wideband, attack, release. Tested: sibilance reduced, low frequencies untouched, finite output. |
| Auto gain | `src/autogain.cpp` (libebur128) | EBU R128 integrated loudness normalisation | `crates/dsp/src/effects/autogain.rs` | Partial | Slow RMS normalisation with target, maximum gain, silence threshold and speed. **Not R128.** Tested: gain converges, capped, silence does not move the gain. |
| Bass enhancer | `src/bass_enhancer.cpp` | Low-band harmonic generation | `crates/dsp/src/effects/bass_enhancer.rs` | Partial | Low-pass split, rectification, DC removal, harmonic blend, output gain. Different algorithm from the reference. Tested: harmonics appear, zero amount is transparent, bounded output. |
| Crystalizer | `src/crystalizer.cpp` | Transient emphasis across bands | `crates/dsp/src/effects/crystalizer.rs` | Partial | Four fixed bands, per-band enable, transient emphasis from fast/slow envelopes, amount and gain. Tested: transients emphasised, steady tones nearly unchanged, disabled bands transparent. |
| Stereo tools | `src/stereo_tools.cpp` | Width, balance, mid/side, stereo base | `crates/dsp/src/effects/stereo_tools.rs` | Partial | Input gain, width 0–200 %, balance, Haas stereo base. No mid/side gain matrix. Tested: mono collapse, double width, balance, delay. |
| Delay | `src/delay.cpp` | L/R delay, feedback, mix | `crates/dsp/src/effects/delay.rs` | Partial | Independent L/R delay, feedback, mix, ping-pong, glided delay time. No tempo sync or tape emulation. Tested: echo timing, feedback decay, dry path. |
| Reverb | `src/reverb.cpp` | Room simulation | `crates/dsp/src/effects/reverb.rs` | Partial | Freeverb-style 8 comb + 4 all-pass network per channel, room size, damping, pre-delay, width, wet/dry. Tested: tail present, tail decays, dry setting transparent, rate retuning, bounded output. |
| Gain | - (per-plugin gain in the reference) | Input/output gain per plugin | `crates/dsp/src/effects/gain.rs` | Implemented | Gain and mute as a chain stage. Tested: −6 dB halves amplitude, mute silences. |
| Autotune | `src/autotune.cpp` | Pitch correction (x42 plugin) | - | Not implemented | Requires a pitch-detection and pitch-shifting engine; a native implementation would be a project of its own. |
| Bass loudness | `src/bass_loudness.cpp` | MDA plugin wrapper | - | Not implemented | Third-party plugin. |
| Convolver | `src/convolver.cpp` | FFT convolution with IRS files | - | Not implemented | Needs an FFT dependency, impulse-response management and a file browser. |
| Crossfeed | `src/crossfeed.cpp` | bs2b-based headphone crossfeed | - | Not implemented | Would need either libbs2b or a reimplementation; deferred. |
| Crosstalk canceller | `src/crosstalk_canceller.cpp` | Stereo crosstalk cancellation | - | Not implemented | Deferred. |
| Crusher | `src/crusher.cpp` | Bit crusher | - | Not implemented | Cheap to add; not part of this increment. |
| Deep noise remover | `src/deepfilternet.cpp` | DeepFilterNet model | - | Not implemented | Requires an ONNX runtime and model downloads. |
| Echo canceller | `src/echo_canceller.cpp` | WebRTC AEC | - | Not implemented | Requires webrtc-audio-processing. |
| Exciter | `src/exciter.cpp` | Harmonic exciter | - | Not implemented | Overlaps with the bass enhancer and crystalizer. |
| Expander | `src/expander.cpp` | Downward expander | - | Not implemented | Shares the detector machinery with the gate; not part of this increment. |
| Mid/side equalizer | `src/midside_equalizer.cpp` | Equalisation in mid/side | - | Not implemented | Deferred. |
| Voice suppressor | `src/voice_suppressor.cpp` | Voice removal | - | Not implemented | Requires a vocal-detection model. |
| Level meter (as an effect) | `src/level_meter.cpp` | Per-plugin metering | `crates/core/src/meter.rs` | Partial | Global input and output meters (peak + RMS) instead of a per-effect meter node. |
| Loudness | `src/loudness.cpp` | EBU R128 metering | - | Not implemented | Would require libebur128 or an independent R128 implementation. |
| Maximizer | `src/maximizer.cpp` | ZamAudio maximizer | - | Not implemented | Third-party plugin. |
| Multiband compressor | `src/multiband_compressor.cpp` | Per-band compression | - | Not implemented | Deferred; the crossover infrastructure would be shared with the crystalizer. |
| Multiband gate | `src/multiband_gate.cpp` | Per-band gating | - | Not implemented | Deferred. |
| Pitch shift | `src/pitch.cpp` | SoundTouch pitch shifting | - | Not implemented | Requires a phase vocoder or SoundTouch. |
| Noise reduction | `src/rnnoise.cpp` | RNNoise | - | Not implemented | Requires the RNNoise model and runtime. |
| Speech processor | `src/speex.cpp` | SpeexDSP | - | Not implemented | Requires speexdsp. |
| LV2/LADSPA hosting | `src/lv2_wrapper.cpp`, `src/ladspa_wrapper.cpp` | Arbitrary plugin loading | - | Not applicable | Deliberately excluded: the project is a self-contained Rust application. |

## 2. Pipeline and application features

| Feature | Reference evidence | Status | Notes |
|---|---|---|---|
| Output pipeline | `src/stream_output_effects.cpp`, `pw_node_manager.cpp` | Implemented | Capture stream acting as `Audio/Sink`, ring buffer, chain, playback stream. |
| Input (microphone) pipeline | `src/stream_input_effects.cpp` | Not implemented | The preset schema has an `output` section only. Documented limitation. |
| Virtual sink | `load_virtual_devices` | Implemented | `pipewire-effects-sink`, `node.virtual`, not auto-connected. Both internal streams opt out of the session manager's restore hooks (`state.restore-props`, `state.restore-target`), so a volume or link target left behind by an earlier session cannot attenuate or retarget the processing path. |
| Virtual source | `load_virtual_devices` | Not implemented | Requires the input pipeline. |
| Effect ordering | `effectsorder.md`, `src/effects_base.cpp` | Implemented | Move up/down in the sidebar; the order is mirrored to the audio thread. |
| Per-effect enable | `PluginBase::bypass` | Implemented | `ChainCommand::SetEnabled`; a disabled slot is skipped without being destroyed. |
| Global bypass | `StreamOutputEffects::set_bypass` | Implemented | Header switch. |
| Preset save/load | `src/presets_manager.cpp` | Implemented | JSON, user directory, atomic writes, validation. |
| Built-in presets | `src/presets_directory_manager.cpp` | Partial | One compiled-in preset (Default). |
| Preset directory precedence | `presets_directory_manager.cpp`, `presets_manager.cpp` | Implemented | The user directory wins over a system directory, which wins over the compiled-in preset. The browser and the loader apply the same precedence, and only user files are deletable. |
| EasyEffects preset import | `src/presets_manager.cpp`, `src/contents/kcfg/*.kcfg` | Partial | EasyEffects `plugins_order` documents are converted on load: supported effects and the parameters with a close equivalent are mapped, and everything else is reported. Section 5 lists the mapping per effect. |
| Preset import/export | `presets_manager` | Implemented | Via `gtk::FileDialog`. |
| Preset autoload per application | `presets_autoload_manager.cpp` | Not implemented | Deferred. |
| Community presets | `presets_community_manager.cpp` | Not implemented | Requires network access; deliberately excluded. |
| Blocklist | `presets_manager.cpp` | Not implemented | Deferred. |
| Last used preset | `DbMain::lastLoadedOutputPreset` | Partial | The loaded preset is shown in-session; it is not persisted across runs. |
| Device selection | `pw_manager.cpp` | Implemented | The destination follows `default.audio.sink` from the session manager's metadata, read before the virtual sink is created; a device picked by hand wins, and *System default* in the *Devices* page returns to the automatic choice. The `priority.session` heuristic stays as the fallback. |
| Following the default device | `bestHardwareOutputDeviceName` | Implemented | The metadata object is watched while the engine runs; a change rebuilds the two streams for the new device. |
| Level meters | `output_level.cpp` | Implemented | Input and output, peak with an RMS marker, 20 Hz refresh. Both meters run through `LevelMeter`, so an empty chain reports the same level at both. |
| Spectrum analyser | `src/spectrum.cpp` | Not implemented | Requires an FFT. |
| Statistics and diagnostics | reference logs + advanced info page | Implemented | Captured/processed frames, underruns, overruns, trims, warnings. |
| Buffer size / samplerate control | `DbMain`, kcfg | Not applicable | The graph rate and quantum are owned by PipeWire and followed, not forced. |
| Translations | `po/` (30+ languages) | Not implemented | English only. |
| Autostart / tray | `src/autostart.cpp` | Not implemented | Deferred. |
| Command line interface | `src/command_line_parser.cpp` | Not implemented | The binary starts the GUI; no flags. |
| Desktop file and icon | `src/contents/*.desktop.in`, icons | Implemented | `data/`. |
| Flatpak / distribution packaging | `util/flatpak`, `PKGBUILD` | Not implemented | Out of scope for this increment. |

## 3. Test coverage in this repository

| Area | Count | Location |
|---|---|---|
| Core primitives (buffers, parameters, chain, meters, math) | 27 | `crates/core/src/**` |
| DSP (biquads, thirteen effects, registry) | 65 | `crates/dsp/src/**`, `crates/dsp/tests/` |
| Engine (config, devices, target resolution, default-sink metadata, processor, stream properties, byte conversion) | 16 | `crates/engine/src/**` |
| Presets (schema, coercion, storage, built-ins, reference conversion) | 83 | `crates/presets/src/**` |
| Interface (formatting, icons, response axis, state model, banners, device labels) | 20 | `crates/ui/src/**` |
| Live PipeWire integration (ignored by default) | 3 | `crates/engine/tests/live_pipewire.rs` |

`cargo test --workspace` therefore runs 211 tests. None of them need an audio
device. Two of the interface tests look up icons in the live icon theme and skip
themselves when no display is available. The three live tests are `#[ignore]`d and
are run explicitly against a running daemon.

## 3.1 Interface verification

The release binary was run under a separate X server with the real PipeWire
daemon and captured with `import` for every primary destination and for the
dialog, empty, warning, bypass, narrow and live-metering states, in the dark and
the light appearance. The states are driven by `PIPEWIRE_EFFECTS_UI_STATE`. The
captures are what found the missing application icon, the two Adwaita-absent icon
names, the squeezed editor column and the two meters disagreeing. See
`docs/architecture.md` section 12.

## 4. Deliberate exclusions and their justification

- **No LV2/LADSPA/plugin hosting.** The reference's most capable effects are
  plugin wrappers. Hosting them would reintroduce the reference's dependency
  graph and would not be a Rust implementation of the effect.
- **No neural models.** Noise suppression and voice removal need model files and
  an inference runtime, which cannot be verified in this environment.
- **No FFT.** Convolution and spectrum display are the only features that need
  one; both are out of this increment rather than faked.
- **No input pipeline.** Adding it would require a second processor, a second
  ring buffer and a virtual source; the current structures are symmetric but the
  work is not done, so it is reported as missing rather than stubbed.

## 5. Reference preset conversion

An EasyEffects document is recognised by the absence of `schema_version` and the
presence of an `output` section, and is converted on load. The table records
exactly what is transferred, and what is reported back to the user when a
reference setting has no equivalent here.

| Reference effect | Reference keys transferred | Converted here as | Reported as left at the default |
|---|---|---|---|
| `filter` | `type`, `frequency`, `quality`, `gain`, `slope` | `mode`, `frequency`, `q`, `gain`, `order` | `mode`, `width`, `decramp`, `equal-mode`, `balance`; unknown types and slopes. The reference default `slope` is `x1`, so an omitted slope becomes order 1. The two ladder types have no exact equivalent: `Ladder-pass` is mapped to a plain low pass and `Ladder-rejection` to a notch, and both are reported. |
| `equalizer` | `left`/`right` `bandN.{frequency,gain,q,type}`, `input-gain`, `output-gain` | ten fixed bands, `output_gain`, shared `bandwidth` (median reference `q`) | `num-bands`, `mode`, `decramp`, `split-channels`, `balance`, `pitch-left`, `pitch-right`; bands that are not bell or shelf, bands folded beyond ten, bands that are muted or not soloed while another band is soloed, and bands with no usable frequency. Both channel blocks are folded into the one shared curve, and a right channel that differs from the left is reported. The per-band `mode`, `slope` and `width` are reported because every band here is one filter with a shared bandwidth. |
| `bass_enhancer` | `scope`, `amount` (dB), `output-gain` | `frequency`, `amount` (linear), `gain` | `harmonics`, `floor`, `floor-active`, `blend`, `listen`. The reference default `amount` of 0 dB becomes the full amount here. |
| `crystalizer` | `output-gain` | `gain` | everything else, including `band0`…`band12`: the reference splits thirteen bands with adaptive intensity, oversampling and a per-band bypass, which is a different algorithm rather than a parameter rescaling. The per-band settings are reported instead of being dropped. |
| `stereo_tools` | `input-gain`, `balance-out`, `side-level` (dB) | `input_gain`, `balance`, `stereo_width` | `output-gain`, `mode`, `balance-in`, `softclip`, `mutel`, `muter`, `phasel`, `phaser`, `side-balance`, `middle-level`, `middle-panorama`, `stereo-base`, `delay`, `sc-level`, `stereo-phase`, `dry`, `wet` |
| `compressor` | `threshold`, `ratio`, `knee`, `attack`, `release`, `makeup`, `wet`, `sidechain.mode` | `threshold`, `ratio`, `knee` (width, positive), `attack`, `release`, `makeup`, `mix`, `detector` | `mode`, `dry`, `release-threshold`, `boost-threshold`, `boost-amount`, `stereo-split`, `hpf-*`, `lpf-*`, the sidechain mix controls; the `Low-Pass` and `SMA` sidechain modes, which have no detector here |
| `limiter` | `threshold`, `release`, `input-gain` | `limit`, `release`, `input_gain` | `output-gain`, `mode`, `oversampling`, `dithering`, `sidechain-type`, `lookahead`, `attack`, `stereo-link`, `alr*`, `gain-boost`, `sidechain-preamp`, the sidechain mix controls |
| `gate` | `curve-threshold`, `reduction`, `attack`, `release`, `wet` | `threshold`, `range` (magnitude), `attack`, `release`, `mix` | `input-gain`, `output-gain`, `dry`, `curve-zone`, `hysteresis`, `hysteresis-threshold`, `hysteresis-zone`, `makeup`, `stereo-split`, `sidechain`, `hpf-mode`, `hpf-frequency`, `lpf-mode`, `lpf-frequency`, `input-to-sidechain`, `input-to-link`, `sidechain-to-input`, `sidechain-to-link`, `link-to-input`, `link-to-sidechain`; the reverse mode is refused because the gate here has no upward mode |
| `deesser` | `f1-freq`, `threshold`, `ratio`, `mode` | `frequency`, `threshold`, `ratio`, `mode` (`Wide` or `Wideband`) | `detection`, `laxity`, `makeup`, `f1-level`, `f2-*`, `sc-listen`. The reference default mode is `Wide`, so a document without one becomes wideband, as it does in the reference. |
| `autogain` | `target`, `silence-threshold` | `target`, `silence_threshold` | `maximum-history`, `reference`, `force-silence`; the reference measures EBU R128 and this auto gain is RMS based |
| `delay` | `time-l`, `time-r`, `mode-l`, `mode-r`, `wet-l`/`wet-r` | `delay_left`, `delay_right`, `mix` | `dry-*`, `sample-*`, `meters-*`, `centimeters-*`, `temperature-*`, `invert-phase-*`; a non-time mode keeps the reference default delay (0 ms) and is reported, and so is a left/right wet pair that differs, because one mix cannot carry both. |
| `reverb` | `predelay`, `decay-time`, `hf-damp`, `amount`, `dry` | `pre_delay`, `room_size` (`decay-time` × 40), `damping` (`hf-damp` ÷ 200), `wet`, `dry` | `room-size`, `diffusion`, `bass-cut`, `treble-cut` |
| any other instance | - | - | the whole instance is named in the warning list |

Two of these conversions are rescalings rather than transfers and are recorded
as such: the reference reverb `decay-time` is a decay in seconds while `room_size`
here is a comb-feedback percentage, and `hf-damp` is a cutoff in hertz while
`damping` here is a percentage. Both are monotonic, and the reference defaults
(`1.5 s`, `5000 Hz`) land on the defaults of this project (`60 %`, `25 %`).

Every converter treats the reference's per-plugin `input-gain` and `output-gain`
the same way. Where this project has an equivalent control, the value is
transferred: the equalizer output gain, the limiter and stereo tools input gain,
and the bass enhancer and crystalizer output gain. Everywhere else both keys are
reported. A reference preset that changes the level of the filter, compressor,
gate, de-esser, auto gain, delay or reverb therefore says so instead of loading at
a different level.

The reference writes an effect's parameters under `<effect>#<instance>` and falls
back to the bare `<effect>` key when reading a document that predates instance
suffixes. The conversion applies the same fallback, so such a document loads here
as it does there.

Values outside this project's parameter ranges are clamped when the preset is
applied rather than rejected, because the reference allows a wider equalizer gain
and a wider compressor ratio than the effects here. Such a value is not
rescaled or dropped silently; it is applied at the limit of the range.

Warnings reach the user in two places: the banner that follows a load or an
import, summarized when there are more than three, and the `Skipped files` row
of the preset list, which shows every warning in full. A family of settings that
differs only by a trailing number, such as the thirteen crystalizer bands, is
summarised as one entry (`band*`) rather than listed key by key. A file whose
name is not a valid preset name is never offered in the browser, and a malformed
file never hides the preset it shadows: it is reported in the `Skipped files`
row, and the entry that is shown is the entry that loads.

Dropping a reference value is only acceptable when this project has no equivalent
for it; where the reference default itself is meaningful, it is honoured. An
omitted filter slope is the reference's `x1` (order 1), an omitted de-esser mode
is the reference's `Wide`, and an omitted delay time is the reference's 0 ms.
