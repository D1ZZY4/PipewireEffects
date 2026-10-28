# Feature matrix

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
| Virtual sink | `load_virtual_devices` | Implemented | `pipewire-effects-sink`, `node.virtual`, not auto-connected. |
| Virtual source | `load_virtual_devices` | Not implemented | Requires the input pipeline. |
| Effect ordering | `effectsorder.md`, `src/effects_base.cpp` | Implemented | Move up/down in the sidebar; the order is mirrored to the audio thread. |
| Per-effect enable | `PluginBase::bypass` | Implemented | `ChainCommand::SetEnabled`; a disabled slot is skipped without being destroyed. |
| Global bypass | `StreamOutputEffects::set_bypass` | Implemented | Header switch. |
| Preset save/load | `src/presets_manager.cpp` | Implemented | JSON, user directory, atomic writes, validation. |
| Built-in presets | `src/presets_directory_manager.cpp` | Implemented | Six compiled-in presets. |
| Preset import/export | `presets_manager` | Implemented | Via `gtk::FileDialog`. |
| Preset autoload per application | `presets_autoload_manager.cpp` | Not implemented | Deferred. |
| Community presets | `presets_community_manager.cpp` | Not implemented | Requires network access; deliberately excluded. |
| Blocklist | `presets_manager.cpp` | Not implemented | Deferred. |
| Last used preset | `DbMain::lastLoadedOutputPreset` | Partial | The loaded preset is shown in-session; it is not persisted across runs. |
| Device selection | `pw_manager.cpp` | Partial | Explicit target device plus a `priority.session` heuristic for "default". Does not read `default.audio.sink` metadata. |
| Following the default device | `bestHardwareOutputDeviceName` | Not implemented | Same metadata limitation. |
| Level meters | `output_level.cpp` | Implemented | Input and output, peak with RMS marker, 20 Hz refresh. |
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
| DSP (biquads, thirteen effects, registry) | 65 | `crates/dsp/src/**` |
| Engine (config, devices, processor, byte conversion) | 12 | `crates/engine/src/**` |
| Presets (schema, coercion, storage, built-ins) | 14 | `crates/presets/src/**` |
| Interface (formatting, response axis, state model) | 9 | `crates/ui/src/**` |
| Live PipeWire integration (ignored by default) | 3 | `crates/engine/tests/live_pipewire.rs` |

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
