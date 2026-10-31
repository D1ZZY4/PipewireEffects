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
| REQUIRED | Thirty effects; the virtual sink and the virtual source; the output and the input pipeline; the destination device; per-effect enable, ordering and global bypass; presets (six built-in, user, import, export, EasyEffects conversion); live meters and the live spectrum; diagnostics; persistence of the device, the preset and the bypass; the GTK4/libadwaita interface; the desktop entry and icon |
| PRESERVE | Preset JSON schema v1 and its reader; the easyeffects conversion vocabulary and its warning list; the existing module boundaries and the realtime rules; the repository identity, licence and documentation |
| EXCLUDED | LV2/LADSPA hosting, because the project is a self-contained Rust application and hosting the reference's plugins would reintroduce its whole dependency graph. Everything else the reference does that is listed below as unfinished is *not* excluded: it is simply not written yet. |
| BLOCKED | Deep noise removal (DeepFilterNet and DPDFNet): the ONNX models and runtime are not present and cannot be fetched here. The community preset browser: it requires network access. Both are external limits of this environment, not scope decisions. |
| UNFINISHED | Features the reference has, that this application is intended to have, that are neither excluded nor blocked, and that are not written: the convolver, the echo canceller, a level-meter effect, a loudness meter, preset autoload profiles per device, the reference's per-channel 30-band equalizer, eight-band multiband effects, the voice suppressor's minimum-kurtosis gate, and the spectrum analyser's display settings. Each is listed with its status in the tables below. None of them is an MVP decision and none is presented as complete. |

The application is therefore complete for the two pipelines it implements -
output and input, on a virtual sink and a virtual source - and is **not** at full
EasyEffects parity. See `docs/architecture.md` for what is verified.

Status values:

- **Implemented** - behaves as described and is covered by tests.
- **Partial** - the capability exists with a documented reduction in scope or
  fidelity.
- **Not started** - intended, absent, and the reason is recorded. It is not an
  exclusion: the work has not been done.
- **Blocked** - intended, and prevented by a precise external limit of this
  environment.
- **Not applicable** - a reference-specific mechanism with no equivalent here,
  with the reason recorded.

Reference evidence refers to files under `references/`.

## 1. Effects

| Effect | Reference evidence | Reference behaviour | Module here | Status | Notes and tests |
|---|---|---|---|---|---|
| Equalizer | `src/equalizer.cpp`, `src/equalizer_apo.cpp`, `docs/plugins/equalizer.md` | 30 bands, per-band type (peak/shelf/pass/notch), frequency, gain, Q; APO import; per-channel split | `crates/dsp/src/effects/equalizer.rs` | Partial | 10 fixed ISO octave bands (31.5 Hz – 16 kHz), shared bandwidth, output gain, analytic response curve. Tested: flat transparency, band boost accuracy against the analytic response, leakage, output gain. |
| Filter | `src/filter.cpp`, `docs/plugins/filter.md` | Low/high/band-pass and band-reject | `crates/dsp/src/effects/filter.rs` | Implemented | Eight shapes (the four above plus notch, all-pass, peak, shelf), order 1–4, frequency, Q, gain. Tested against measured pass/stop bands, slope growth with order, notch rejection, stability. |
| Compressor | `src/compressor.cpp`, `docs/plugins/compressor.md` | Feed-forward compressor, peak/RMS detector, ratio, knee, attack/release, makeup, a high/low-pass filtered sidechain | `crates/dsp/src/effects/compressor.rs` | Partial | Threshold, detector, ratio, knee, attack, release, makeup, mix, stereo-linked detector, and a sidechain high-pass and low-pass of 12, 24 or 36 dB/oct that keep rumble or hiss out of the detector. An *external* sidechain input is not possible: the reference takes it from a probe port on its filter node and this application's effects run in a software chain without a second audio input. Tested: below-threshold transparency, ratio accuracy, makeup, dry/wet, and that engaging the sidechain high-pass stops a 60 Hz tone from triggering the compressor. |
| Limiter | `src/limiter.cpp`, `src/limiter.hpp`, `docs/plugins/limiter.md` | LSP lookahead limiter (mode, oversampling, dithering, lookahead, attack, release, threshold, gain boost, stereo link, ALR) | `crates/dsp/src/effects/limiter.rs` | Partial | A peak limiter with a real lookahead: the audio is delayed by the lookahead, the gain envelope descends linearly over that window so a transient is attenuated *before* it reaches the output, and a per-sample requirement clamps the result so the ceiling holds by construction. Parameters: input gain, limit, lookahead (0.1–20 ms), release, stereo link (0–100 %). The latency is reported to the diagnostics. The reference's LSP modes, oversampling, dithering, ALR, gain boost, sidechain and attack controls are not implemented. Tested: the ceiling is never exceeded with +20 dB of drive, a limited burst keeps its sine shape (its crest factor stays at √2 instead of being clipped), the output lags by exactly the lookahead, a 1.0 lookahead still passes quiet material at unity, the gain recovers after a transient, and a 0 % stereo link limits only the loud channel. |
| Gate | `src/gate.cpp`, `docs/plugins/gate.md` | LSP gate with a curve zone, hysteresis, reduction, attack, release and a filtered sidechain | `crates/dsp/src/effects/gate.rs` | Partial | Threshold, range, curve (a transition zone below the threshold, so the reduction ramps instead of switching), hysteresis (a close threshold below the open one), attack, hold, release, mix and a sidechain high-pass and low-pass of 12, 24 or 36 dB/oct that keep rumble or hiss out of the detector; both filters are off by default, so a preset written before this change is unchanged. No *external* sidechain input is possible for the same reason as the compressor's. The reference's `hysteresis-threshold`/`hysteresis-zone` pair is reported rather than guessed at, because the reference delegates the gate's shape to the LSP plugin's ports and their exact convention is not verifiable from this checkout. Tested: a loud signal stays open, a quiet signal falls to the range, hold delays the close, the curve softens the transition, hysteresis keeps the gate open down to the close threshold and no further, and a 2 kHz 36 dB/oct sidechain high-pass stops a 250 Hz rumble from opening the gate. |
| De-esser | `src/deesser.cpp`, `docs/plugins/deesser.md` | Sibilance reduction, split or wideband | `crates/dsp/src/effects/deesser.rs` | Implemented | Band detector, threshold, ratio, split/wideband, attack, release. Tested: sibilance reduced, low frequencies untouched, finite output. |
| Auto gain | `src/autogain.cpp` (libebur128) | EBU R128 integrated loudness normalisation | `crates/dsp/src/effects/autogain.rs`, `crates/dsp/src/r128.rs` | Implemented | ITU-R BS.1770 K-weighting (the standard shelf and RLB high-pass, designed for the running sample rate), 400 ms blocks at a 100 ms hop, the absolute −70 LUFS and relative −10 LU gates, and the −0.691 offset. The measurement covers a rolling 30-second window rather than the whole program. Parameters: target loudness (LUFS), maximum gain, silence threshold, speed. Tested: a full-scale sine measures about 0 LUFS and a −23 dBFS sine measures −23 LUFS within 0.4 LU, a 10 dB step moves the measurement by 10 LU, silence is not measured, the gain converges and is capped. |
| Bass enhancer | `src/bass_enhancer.cpp` | Low-band harmonic generation | `crates/dsp/src/effects/bass_enhancer.rs` | Partial | Low-pass split, rectification, DC removal, harmonic blend, output gain. Different algorithm from the reference. Tested: harmonics appear, zero amount is transparent, bounded output. |
| Crystalizer | `src/crystalizer.cpp`, `src/crystalizer_preset.cpp`, `src/contents/kcfg/easyeffects_db_crystalizer.kcfg` | Transient emphasis across thirteen bands, each with an intensity in decibels, a mute and a bypass, plus adaptive intensity, oversampling and a transition band | `crates/dsp/src/effects/crystalizer.rs` | Partial | Thirteen fixed bands split by a low-pass cascade at 32 Hz to 16 kHz, each with its own intensity in decibels (−40 to 32), *Mute* and *Bypass*, plus an output gain. The intensity is the gain applied to a band in proportion to how far its fast envelope leads its slow one, so a steady tone is untouched and 0 dB is transparent. The reference's `adaptive-intensity`, `oversampling`, `oversampling-quality`, `use-fixed-quantum` and `transition-band` are reported rather than approximated. The thirteen `bandN` blocks of a reference preset are converted band by band. Tested: there are thirteen bands and every identifier is published, transients are emphasised, muting a band removes it, a bypassed band is not emphasised, a steady tone is barely changed, and the output stays finite under abuse. |
| Stereo tools | `src/stereo_tools.cpp` | The Calf Stereo Tools controls: seven routing modes, middle/side levels and panoramas, balances, phase and mute per channel, soft clip, stereo base | `crates/dsp/src/effects/stereo_tools.rs` | Partial | All seven routing modes (stereo, stereo to mid/side, mid/side to stereo, mono left, mono right, mono sum, swap), input and output balance, per-channel phase and mute, middle and side level, middle panorama, side balance, width 0–200 %, a Haas delay and soft clip. `stereo-phase` and `sc-level` are reported rather than approximated, because they are the Calf plugin's own all-pass and side-chain controls and their exact mapping is not verifiable from this checkout. Tested: mono collapse, double width, independent middle and side levels, the mono-sum and channel-swap modes, the mid/side round trip, output balance, per-channel phase and mute, soft clip, and the Haas delay. |
| Delay | `src/delay.cpp` | L/R delay, feedback, mix | `crates/dsp/src/effects/delay.rs` | Partial | Independent L/R delay, feedback, mix, ping-pong, glided delay time. No tempo sync or tape emulation. Tested: echo timing, feedback decay, dry path. |
| Reverb | `src/reverb.cpp` | Room simulation with a character preset, decay time, damping, diffusion, a tone stage and a tank mix | `crates/dsp/src/effects/reverb.rs` | Partial | Freeverb-style 8 comb + 4 all-pass network per channel, room size, damping, diffusion (the all-pass feedback), pre-delay, a high-pass and a low-pass on the wet signal, width and wet/dry. The reference's `room-size` character preset and its `decay-time` are one control here, rescaled, and the tank mix is not exposed. Tested: tail present, tail decays, dry setting transparent, rate retuning, per-channel pre-delay isolation, and that the tone stage narrows the tail. |
| Gain | - (per-plugin gain in the reference) | Input/output gain per plugin | `crates/dsp/src/effects/gain.rs` | Implemented | Gain and mute as a chain stage. Tested: −6 dB halves amplitude, mute silences. |
| Autotune | `src/autotune.cpp`, `src/autotune_preset.cpp` | Pitch correction (x42 fat1): correction speed, tuning reference, scale mask, bend range, filter, fast mode | `crates/dsp/src/effects/autotune.rs` | Partial | A chromatic corrector built on the shared phase vocoder: a normalised autocorrelation over the last 1024 samples finds the pitch inside *Lowest pitch*…*Highest pitch*, the nearest MIDI note against *Reference pitch* gives a target, and the ratio glides towards it at *Speed*, scaled by *Strength*, before the vocoder resynthesises. Parameters: *Speed* (5–200 ms), *Strength*, *Lowest pitch*, *Highest pitch*, *Reference pitch* (420–460 Hz) and *Mix*. The reference's scale mask, bend range, filter, fast mode, bias and per-plugin gains have no equivalent here and are reported; the reference's `correction` and `tuning` keys are transferred. Tested: a 226 Hz tone is pulled to A3 (220 Hz), zero strength leaves the pitch alone, a zero mix is transparent, and the output stays finite at the widest settings. |
| Harmonizer | `src/harmonizer.cpp`, `src/harmonizer_preset.cpp` | Two shifted voices with per-voice interval and gain, a chord mode and a dry/wet control | `crates/dsp/src/effects/harmonizer.rs` | Partial | Two phase-vocoder voices run in parallel over the same input and are summed with the dry signal: *Voice 1* and *Voice 2* each have a semitone interval (−24…+24) and a level, and *Mix* sets how much of the pair replaces the dry signal. The reference's `interval-left`/`interval-right`, `gain-left`/`gain-right` and `dry-wet` are transferred; its chord mode, note and chord selection are reported. Tested: an octave-up voice is generated at twice the frequency, a fifth voice at 659 Hz, a zero mix is transparent, and the output stays finite at the extremes. |
| Bass loudness | `src/bass_loudness.cpp` | MDA LV2 *Loudness* wrapper (`loudness`, `output`, `link`) | `crates/dsp/src/effects/bass_loudness.rs` | Partial | An equal-loudness contour built from a low shelf at 120 Hz and a high shelf at 6 kHz whose gains grow as the source level falls below full scale, plus the static *Output* trim and the *Link* compensation, which lowers the output further as the source falls. The reference's `-100..0 dB` range is clamped to `-60..0 dB` here. The MDA transfer function is not reproduced: the contour is approximated from the ISO 226 anchors, so the mapping of the three controls is a native design rather than a translation, and the exact plugin curve is not verifiable from this checkout. Tested: a quiet source lifts 60 Hz more than 3 dB above the midrange, a full-level source leaves the contour flat within 0.6 dB, the output trim lands on its decibels, the link setting lowers a 1 kHz tone by more than 4 dB at a 50 dB source level, and the output stays finite at the extremes. A live test on the real daemon confirms it end to end: `crates/engine/tests/live_bass_loudness.rs` lifted a 60 Hz tone from −20.00 dB to −8.15 dB over 113 152 processed frames with no overruns or trims, and its editor was captured in both appearances. |
| Convolver | `src/convolver.cpp` | FFT convolution with IRS files | `crates/dsp/src/fft.rs` | Not started | The FFT primitive exists here; partitioned convolution, impulse-response storage and the file browser were not written. |
| Deep noise remover | `src/deepfilternet.cpp` | DeepFilterNet model | - | Blocked | Needs an ONNX runtime and model files that are not present and cannot be fetched here. |
| Echo canceller | `src/echo_canceller.cpp` | SpeexDSP / WebRTC AEC | - | Not started | `webrtc-audio-processing` 1.3 and `speexdsp` 1.2.1 are both installed, so this is not environment-blocked; a binding and the delay and alignment handling were not written. |
| Level meter (as an effect) | `src/level_meter.cpp` | Per-plugin metering | `crates/core/src/meter.rs` | Partial | Global input and output meters (peak + RMS) instead of a per-effect meter node. |
| Loudness | `src/loudness.cpp`, `src/contents/kcfg/easyeffects_db_loudness.kcfg` | LSP Loudness Compensator: an equal-loudness contour driven by an output volume, hard clipping with a clipping range, and five contour standards | `crates/dsp/src/effects/loudness.rs` | Partial | An equal-loudness contour built from a low shelf at 120 Hz and a high shelf at 6 kHz whose gains grow as the output volume falls, with the five standards (Flat, ISO 226:2003, Fletcher-Munson, Robinson-Dadson, ISO 226:2023) selecting different ceilings and scales, and *Clipping* with *Clipping range* applying a hard ceiling. The contour is an IIR biquad cascade, so the reference's FFT size and its FFT/IIR mode are reported rather than exposed, and there is no per-effect loudness meter in the interface. The R128 momentary and short-term windows live in `crates/dsp/src/r128.rs` and are tested directly. Tested: a full volume keeps the contour flat within 0.5 dB, a quiet volume lifts 60 Hz more than 3 dB above the midrange, the Flat standard disables the contour, Fletcher-Munson lifts the bass further than ISO 226:2003, clipping holds its ceiling, and the output stays finite at the extremes. |
| Multiband compressor | `src/multiband_compressor.cpp` | Per-band compression | `crates/dsp/src/effects/multiband_compressor.rs` | Partial | Up to four Linkwitz-Riley bands with a per-band threshold, ratio, knee, attack, release, makeup and enable, plus a global mix and three split frequencies. The reference splits up to eight bands and adds sidechain modes, lookahead, boost and solo/mute. Tested: a unity ratio is transparent, loud material is reduced, makeup lifts the result, a disabled band passes through. |
| Multiband gate | `src/multiband_gate.cpp` | Per-band gating | `crates/dsp/src/effects/multiband_gate.rs` | Partial | Up to four Linkwitz-Riley bands with a per-band threshold, range, attack, hold, release, makeup and enable. The reference splits up to eight bands and adds hysteresis, solo/mute and sidechain modes. Tested: a loud signal stays open, a quiet signal is closed by the range, a disabled band is not gated. |
| Voice suppressor | `src/voice_suppressor.cpp` | Voice removal | `crates/dsp/src/effects/voice_suppressor.rs` | Partial | STFT with a 1024-point FFT and 75 percent overlap; a bin is treated as voice when its inter-channel correlation and phase difference cross the thresholds, inside the start and end frequencies, with an inverted mode that keeps the voice instead. The instantaneous-frequency gate is applied: a bin counts only if its instantaneous frequency, recovered from the phase advance with the bin's expected advance added back, is at or below *Maximum instantaneous frequency*. The reference's minimum-kurtosis gate is still reported and not applied, because a per-bin temporal kurtosis estimate is not implemented. Real and inverted modes, and the frequency gate, are tested. |
| Pitch shift | `src/pitch.cpp`, `src/pitch_preset.cpp`, `src/contents/kcfg/easyeffects_db_pitch.kcfg` | SoundTouch pitch shifting (mode, formant, transients, detector, phase, cents, semitones, octaves, tempo and rate difference) | `crates/dsp/src/effects/pitch_shift.rs` | Partial | A phase-vocoder pitch shifter: a 2048-point Hann-windowed STFT at 75 % overlap, per-bin instantaneous frequency recovered from the phase advance, the bins remapped by the shift ratio and the synthesis phases propagated, summed back by overlap-add. Parameters: semitones (±24), cents (±100) and mix. The algorithmic latency of 2048 frames is declared and reported to the diagnostics, and the first frames of a fresh chain are the analysis window's own fill rather than the input. The reference's SoundTouch presets (formant preservation, transient handling, detector, phase mode, quick seek, anti alias, sequence/seek/overlap lengths, tempo and rate difference) have no equivalent here and are reported. Tested: an octave up doubles a 1 kHz tone's frequency, an octave down halves a 2 kHz one, a zero shift keeps the frequency, +100 cents reaches 1059 Hz, an impulse lands at the declared latency, a zero mix is bit-identical to the input, and the output stays finite at the widest settings. A live test on the real daemon confirms it end to end: `crates/engine/tests/live_pitch_shift.rs` moved a 1 000 Hz tone's loudest band from 1 047 Hz to 2 149 Hz at +12 semitones over 184 320 processed frames with no overruns or trims, and its editor was captured in both appearances. |
| Noise reduction | `src/rnnoise.cpp`, `src/rnnoise.hpp`, `docs/plugins/rnnoise.md` | RNNoise suppression of stationary noise, with VAD, a wet mix and a loadable custom model | `crates/dsp/src/effects/noise_reduction.rs` | Partial | The standard model through `nnnoiseless` 0.5.2, which is a Rust port of RNNoise rather than a binding to `librnnoise`. It processes the 480-sample frames RNNoise is defined for, so it runs only at 48 kHz; at another graph rate the stage passes the signal through unchanged. Parameters: input gain, output gain, mix, VAD gate and its threshold. No custom `.rnn` model, one instance per channel, no resampling, and no release control: the reference's `release` grace period is a fixed smoothing constant here. Tested: an unsupported rate is transparent, the first frame is the dry input, a zero mix is transparent, the output gain scales, and the output is finite under abuse. Also the only effect with its own live test, which runs it in the real chain on a private sink and asserts that a −20 dB tone leaves the chain more than 20 dB lower with no dropouts beyond the ring's initial fill. |
| Speech processor | `src/speex.cpp`, `src/contents/kcfg/easyeffects_db_speex.kcfg` | SpeexDSP preprocessor: VAD, AGC and denoising, with a dereverb stage | `crates/dsp/src/effects/speech_processor.rs` | Partial | A native speech stage in one pass: a noise floor tracked by a fast-fall/slow-rise envelope, voice activity from the level above that floor with an attack and a release, a downward noise reduction proportional to how far the signal is from voice, and an automatic gain driven by the R128 short-term loudness towards a target with a maximum. Parameters: *Voice activity detection*, *Voice threshold above the floor*, *Voice release*, *Automatic gain*, *Target loudness*, *Maximum gain*, *Noise reduction*, *Reduction* and *Mix*. It is not SpeexDSP: the reference's `vad.probability-start`/`probability-continue` are reported because this stage decides on a level, `enable-dereverb` is reported because no dereverb is implemented, and the per-plugin gains are reported. Tested: a steady hiss is reduced when nothing speaks, the denoiser can be switched off, a quiet voice is lifted towards the target, a zero mix is transparent, and the output stays finite at the extremes. |
| LV2/LADSPA hosting | `src/lv2_wrapper.cpp`, `src/ladspa_wrapper.cpp` | Arbitrary plugin loading | - | Not applicable | Deliberately excluded: the project is a self-contained Rust application. |

## 2. Pipeline and application features

| Feature | Reference evidence | Status | Notes |
|---|---|---|---|
| Output pipeline | `src/stream_output_effects.cpp`, `pw_node_manager.cpp` | Implemented | Capture stream acting as `Audio/Sink`, ring buffer, chain, playback stream. |
| Input (microphone) pipeline | `src/stream_input_effects.cpp` | Implemented | Capture stream reading the chosen input device, ring buffer, a second chain, and a playback stream publishing `pipewire-effects-source` as an `Audio/Source`. Opt-in: the engine starts only when *Process microphone input* is on. Covered by a live test that records from the virtual source with `pw-cat` and asserts the chain ran. |
| Virtual sink | `load_virtual_devices` | Implemented | `pipewire-effects-sink`, `node.virtual`, not auto-connected. Both internal streams opt out of the session manager's restore hooks (`state.restore-props`, `state.restore-target`), so a volume or link target left behind by an earlier session cannot attenuate or retarget the processing path. |
| Virtual source | `load_virtual_devices` | Implemented | `pipewire-effects-source`, an `Audio/Source` with `node.virtual`, published by the input pipeline. Applications pick it as their recording device. |
| Effect ordering | `effectsorder.md`, `src/effects_base.cpp` | Implemented | Move up/down in the sidebar; the order is mirrored to the audio thread. |
| Per-effect enable | `PluginBase::bypass` | Implemented | `ChainCommand::SetEnabled`; a disabled slot is skipped without being destroyed. |
| Global bypass | `StreamOutputEffects::set_bypass` | Implemented | Header switch. |
| Preset save/load | `src/presets_manager.cpp` | Implemented | JSON, user directory, atomic writes, validation. |
| Built-in presets | `src/presets_directory_manager.cpp` | Implemented | Six compiled-in presets: *Default*, *Podcast voice*, *Music, speakers*, *Headphones*, *Bass boost* and *Clean microphone*. They are compiled into the binary so a fresh installation has a usable library without a system preset directory, `make install` also writes them into the system directory under their preset names, and a test asserts that every built-in parses, names only effects that exist in the registry, and uses only parameters those effects declare. |
| Preset directory precedence | `presets_directory_manager.cpp`, `presets_manager.cpp` | Implemented | The user directory wins over a system directory, which wins over the compiled-in preset. The browser and the loader apply the same precedence, and only user files are deletable. |
| EasyEffects preset import | `src/presets_manager.cpp`, `src/contents/kcfg/*.kcfg` | Partial | EasyEffects `plugins_order` documents are converted on load: supported effects and the parameters with a close equivalent are mapped, and everything else is reported. Section 5 lists the mapping per effect. |
| Preset import/export | `presets_manager` | Implemented | Via `gtk::FileDialog`. |
| Preset autoload per device | `presets_autoload_manager.cpp` | Not started | Deferred: no work on this has started. The reference stores `{device, device-description, device-profile, preset-name}` profiles and loads the matching preset when a device or route is selected; the equivalent here would watch the pipeline's resolved device and load its profile. |
| Community presets | `presets_community_manager.cpp` | Blocked | Requires network access; deliberately excluded. |
| Blocklist | `presets_manager.cpp` | Not applicable | The reference uses the blocklist to keep named applications out of its processing when it captures streams itself. This application processes whatever the user routes into its own virtual sink, so there is no per-application capture to block: excluding an application is done by routing it elsewhere, which is a session-manager decision this application does not make on the user's behalf. |
| Last used preset | `DbMain::lastLoadedOutputPreset` | Implemented | The loaded preset is written to the settings file and restored on the next start. |
| Device selection | `pw_manager.cpp` | Implemented | Each pipeline follows its own metadata key (`default.audio.sink`, `default.audio.source`), read before the virtual node is created; a device picked by hand wins, is persisted, and *System default* returns to the automatic choice. The `priority.session` heuristic stays as the fallback. |
| Following the default device | `bestHardwareOutputDeviceName` | Implemented | The metadata object is watched while the engine runs; a change rebuilds the two streams for the new device. |
| Level meters | `output_level.cpp` | Implemented | Input and output, peak with an RMS marker, 20 Hz refresh, for whichever pipeline is being edited. Both meters run through `LevelMeter`, so an empty chain reports the same level at both. |
| Spectrum analyser | `src/spectrum.cpp`, `src/spectrum_dsp.cpp`, `src/contents/kcfg/easyeffects_db_spectrum.kcfg` | Partial | `crates/dsp/src/spectrum.rs`, `crates/engine/src/spectrum.rs`, `crates/ui/src/spectrum.rs`. The reference is a live FFT analyser over the processed audio, with a selectable shape, point count, height, frequency range, logarithmic axis, FFT size and automatic level range. Here it is a fixed 48-band logarithmic bar display from 20 Hz to 20 kHz, measured from the mid signal with an 8192-point Hann-windowed transform every third block — the reference's own default FFT size — and published through atomics. Each band is the integral of the Parseval-corrected bin power between its edges, normalised to the analysis bandwidth when the band is narrower than the window's resolution, so a band sums the tones inside it and a full-scale tone reads 0 dB. The reference's shape, point count, height, frequency range, level range and window settings are not exposed, so the display is fixed at the reference's default resolution and window. Tested: a tone lands in its band at its level, full scale reads 0 dB, two tones inside a band sum, a sub-resolution tone still reads its level, silence stays at the floor, the top bands of an 8 kHz graph stay at the floor, most bands are resolved, a short history reports nothing, the band centres rise and span the audio range, the published snapshot round-trips, a cleared engine reads the floor, and a running chain reports a tone at its own level and frequency. |
| Statistics and diagnostics | reference logs + advanced info page | Implemented | Captured/processed frames, underruns, overruns, trims, ring occupancy, the chain's reported latency, detected compute devices, GPU blocks/fallbacks/disagreements and warnings, per pipeline. Shown in a *Diagnostics* dialog opened from the main menu, and the menu entry exists only while *Developer mode* is on. |
| Developer mode | reference advanced info page | Implemented | A switch in Preferences adds a *Diagnostics* entry to the main menu; the entry is absent while the switch is off. The choice is persisted in the settings file. |
| Processing backend | no reference equivalent (the reference is CPU-only) | Implemented | `Preferences → Processing backend` offers CPU, GPU and Both, with Both as the built-in default. GPU runs the elementwise gain stage on a Vulkan compute device and keeps recursive/spectral effects on the CPU; Both adds a per-block CPU reference and replaces a disagreeing result. The choice is persisted as `audio_backend`, applies to both pipelines, and restarts the running engines. The wait is bounded by half the block duration and a miss falls back to the CPU; a machine with no Vulkan device keeps CPU with a warning. Verified by two live tests against a real daemon and a private sink, and by the accelerator unit tests. |
| Main menu | reference hamburger menu | Implemented | *Devices*, *Preferences*, *Diagnostics*, *About PipeWire Effects* and *Quit*. Devices, Preferences and Diagnostics open `adw::PreferencesDialog`s; `Ctrl`+`4`, `Ctrl`+`,` and `Ctrl`+`5` activate them. *Diagnostics* is hidden unless Developer mode is on. |
| Top-level navigation | reference device/page tabs | Implemented | An *Output* / *Input* switcher in the header (`Ctrl`+`1`, `Ctrl`+`2`) selects the pipeline; each owns its sidebar, meters and editor. The preset library lives in each pipeline's sidebar rather than on a page of its own. |
| Adaptive layout | reference mobile layout | Implemented | Below 860 px the sidebar collapses into a drawer with a back button, the switcher switches to its narrow policy, and the window's minimum size is 360x480. Captured at 720x620 and 360x740 in both appearances. |
| Buffer size / samplerate control | `DbMain`, kcfg | Not applicable | The graph rate and quantum are owned by PipeWire and followed, not forced. |
| Settings persistence | `DbMain`, `db_manager.cpp` | Implemented | `$XDG_CONFIG_HOME/pipewire-effects/settings.json`, written atomically; a damaged file falls back to the defaults with a message. |
| Stream failure recovery | reference reconnect logic | Implemented | A failed stream is rebuilt with an increasing backoff, and the Devices page offers a manual *Reconnect*. |
| Translations | `po/` (30+ languages) | Implemented | gettext with a `po/` catalog (`po/POTFILES.in`, `po/LINGUAS`, `po/pipewire-effects.pot`) and `scripts/update-po.sh`. Every user-facing string is wrapped with `pwe_core::i18n::t`/`tr`/`tn`; effect labels, parameter labels, groups and choice labels are translated when read, and the vocabulary they name is marked for extraction (`pwe_core::i18n::mark`) so the template lists it. No translation ships yet, so an installed build shows English until a `po/<lang>.po` is added. |
| Autostart | `src/autostart.cpp` | Implemented | *Start on login* writes the XDG autostart entry with `--no-window`, so processing begins at sign-in and launching the application again opens the running instance's window. |
| Tray icon | `src/main.cpp` (`QSystemTrayIcon`) | Not applicable | The reference is KDE-first and uses Qt's system tray. GNOME Shell provides no system tray, so on this target a tray icon would need a StatusNotifierItem implementation and a shell extension to be visible at all. *Start on login* plus the single-instance activation covers what the tray is used for there. |
| Command line interface | `src/command_line_parser.cpp` | Implemented | `--help`, `--version`, `--preset`, `--device`, `--no-window`, `--list-presets`, `--list-devices`. |
| Desktop file and icon | `src/contents/*.desktop.in`, icons | Implemented | `data/`. |
| Flatpak / distribution packaging | `util/flatpak`, `PKGBUILD` | Partial | `make install`, a `PKGBUILD` and a Flatpak manifest exist and the `make install` tree was inspected; no package was built or installed into the system here. |

## 3. Test coverage in this repository

| Area | Count | Location |
|---|---|---|
| Core primitives (buffers, parameters, chain, meters, math, pipeline kinds, gettext lookup) | 37 | `crates/core/src/**` |
| DSP (biquads, the FFT, the spectrum analyser, the sidechain filter, the Linkwitz-Riley crossover, thirty effects, registry, R128 measurement) | 197 | `crates/dsp/src/**`, `crates/dsp/tests/` |
| Engine (config, devices, target resolution, metadata, processor, chain latency, spectrum snapshot, stream properties, byte conversion, registry filtering, Vulkan probe/accelerator) | 38 | `crates/engine/src/**` |
| Presets (schema, coercion, storage, built-ins and their registry check, reference conversion for both sections) | 142 | `crates/presets/src/**` |
| Settings (model, atomic store, autostart entry) | 11 | `crates/services/src/**` |
| Interface (formatting, icons, response axis, spectrum axis, state model, banners, device labels) | 30 | `crates/ui/src/**` |
| Command line (parsing, help, exported presets) | 4 | `crates/app/src/cli.rs` |
| Live integration (seven PipeWire, two Vulkan backend, ignored by default) | 9 | `crates/engine/tests/live_*.rs` |

`cargo test --workspace` therefore runs 466 tests. None of them need an audio
device. Two of the interface tests look up icons in the live icon theme and skip
themselves when no display is available. The nine live tests, the CPU baseline
and the GPU timing measurement are `#[ignore]`d and are run explicitly against a
running daemon.

## 3.1 Interface verification

The release binary ran in a headless GNOME Wayland session
(`gnome-shell --headless --wayland --virtual-monitor`, in a private D-Bus
session, with `GDK_BACKEND=wayland`) and every primary destination and the
dialog, empty, warning, bypass and narrow states were captured through
`PIPEWIRE_EFFECTS_UI_CAPTURE`, which renders the window's own render tree. GNOME
refuses `org.gnome.Shell.Screenshot` to other clients, so a compositor capture
was not available; the app renders itself instead. `ADW_DEBUG_COLOR_SCHEME`
selected the light and the dark appearance. The states are driven by
`PIPEWIRE_EFFECTS_UI_STATE`.

The captures are what found the missing application icon, the two
Adwaita-absent icon names, the squeezed editor column and the two meters
disagreeing. The Wayland pass found that the `narrow` state never resized the
window (GTK4 ignores a size set after mapping) and that the About dialog is a
separate toplevel, and verified the icon redraw, checked at 32, 64, 128 and
512 px and in the About dialog. See `docs/architecture.md` section 12.

A further pass inspected the accessibility tree over the real AT-SPI bus and
took an independent whole-screen capture under `Xvfb` with ImageMagick's
`import`, which is a screen capture by another process rather than the window
rendering itself. It found that the crystalizer's four band switches were
labelled with their parameter identifiers (`band1`…`band4`); they now read
`Below 200 Hz`, `200 Hz to 2 kHz`, `2 kHz to 8 kHz` and `Above 8 kHz`, and the
labels are marked for translation. Three conversion warnings that were written
as bare string literals instead of going through gettext were found and
translated. The accessibility tree named every interactive element and reported
the parameter sliders' value text; see `docs/architecture.md` section 11.

The final pass added the twenty-fifth and twenty-sixth effects and re-ran the
harness on the isolated headless session: 25 light states and 19 dark states,
including both new editors in both appearances, with no failed capture. A tone
fed through the virtual sink read `−41.1 dB` at the input meter, `−44.0 dB` at
the output meter and `441 Hz −44.1 dB` in the spectrum caption, and the
Diagnostics capture of the `Both` backend read 145 GPU blocks, 20 fallbacks and
0 disagreements on the Intel HD 620. The measured results and the GPU
fallback explanation are in `docs/architecture.md` sections 12 and 13.

## 4. Deliberate exclusions and their justification

- **No LV2/LADSPA/plugin hosting.** The reference's most capable effects are
  plugin wrappers. Hosting them would reintroduce the reference's dependency
  graph and would not be a Rust implementation of the effect.
- **One neural effect, through a Rust port.** Noise reduction is RNNoise as
  reimplemented by `nnnoiseless`, so it is ordinary Rust code with no C library,
  model file or inference runtime to install. The remaining neural effects
  (deep noise removal and voice removal) still need model files and an ONNX
  runtime, which cannot be verified in this environment.
- **A hand-written FFT, not a dependency.** `crates/dsp/src/fft.rs` is an
  iterative radix-2 transform, checked against a bin-centred tone, a forward and
  inverse round trip and Parseval. It exists because the voice suppressor and the
  spectrum need one; it replaced the earlier choice to defer both.
  `nnnoiseless` carries its own FFT (`realfft` over `rustfft`) internally, which
  is the dependency's own business and is not used by this project's code.

## 5. Reference preset conversion

An EasyEffects document is recognised by the absence of `schema_version` and the
presence of an `output` section, and is converted on load. The table records
exactly what is transferred, and what is reported back to the user when a
reference setting has no equivalent here.

| Reference effect | Reference keys transferred | Converted here as | Reported as left at the default |
|---|---|---|---|
| `filter` | `type`, `frequency`, `quality`, `gain`, `slope` | `mode`, `frequency`, `q`, `gain`, `order` | `mode`, `width`, `decramp`, `equal-mode`, `balance`; unknown types and slopes. The reference default `slope` is `x1`, so an omitted slope becomes order 1. The two ladder types have no exact equivalent: `Ladder-pass` is mapped to a plain low pass and `Ladder-rejection` to a notch, and both are reported. |
| `equalizer` | `left`/`right` `bandN.{frequency,gain,q,type}`, `input-gain`, `output-gain` | ten fixed bands, `output_gain`, shared `bandwidth` (median reference `q`) | `num-bands`, `mode`, `decramp`, `split-channels`, `balance`, `pitch-left`, `pitch-right`; bands that are not bell or shelf, bands folded beyond ten, bands that are muted or not soloed while another band is soloed, bands with no usable frequency, and bands that fall outside the ten fixed bands. Both channel blocks are folded into the one shared curve, and a right channel that differs from the left is reported. The per-band `mode`, `slope` and `width` are reported because every band here is one filter with a shared bandwidth. |
| `bass_enhancer` | `scope`, `amount` (dB), `output-gain` | `frequency`, `amount` (linear), `gain` | `harmonics`, `floor`, `floor-active`, `blend`, `listen`. The reference default `amount` of 0 dB becomes the full amount here. |
| `crystalizer` | `output-gain` | `gain` | everything else, including `band0`…`band12`: the reference splits thirteen bands with adaptive intensity, oversampling and a per-band bypass, which is a different algorithm rather than a parameter rescaling. The per-band settings are reported instead of being dropped. |
| `bass_loudness` | `loudness`, `output`, `link` | `loudness`, `output`, `link` | `inputGain`, `outputGain`; a value below the `-60 dB` floor here is clamped to it rather than rescaled |
| `stereo_tools` | `input-gain`, `balance-in`, `balance-out`, `softclip`, `mutel`, `muter`, `phasel`, `phaser`, `mode`, `side-level`, `side-balance`, `middle-level`, `middle-panorama`, `delay` | every control of the module except the two below | `output-gain`, `stereo-base`, `sc-level`, `stereo-phase`, `dry`, `wet`; the phase rotation and the side-chain level are the Calf plugin's own and their mapping is not verifiable here |
| `compressor` | `threshold`, `ratio`, `knee`, `attack`, `release`, `makeup`, `wet`, `sidechain.mode`, `hpf-mode`, `hpf-frequency`, `lpf-mode`, `lpf-frequency` | `threshold`, `ratio`, `knee` (width, positive), `attack`, `release`, `makeup`, `mix`, `detector`, the two sidechain filters | `mode`, `dry`, `release-threshold`, `boost-threshold`, `boost-amount`, `stereo-split`, the sidechain mix controls; the `Low-Pass` and `SMA` sidechain modes, which have no detector here |
| `limiter` | `threshold`, `release`, `lookahead`, `stereoLink`, `input-gain` | `limit`, `release`, `lookahead`, `stereo_link`, `input_gain` | `output-gain`, `mode`, `oversampling`, `dithering`, `sidechain-type`, `attack`, `alr*`, `gain-boost`, `sidechain-preamp`, the sidechain mix controls |
| `gate` | `curve-threshold`, `curve-zone`, `reduction`, `attack`, `release`, `wet` | `threshold`, `range` (magnitude), `curve` (the zone width), `attack`, `release`, `mix` | `input-gain`, `output-gain`, `dry`, `hysteresis`, `hysteresis-threshold`, `hysteresis-zone`, `makeup`, `stereo-split`, `sidechain`, `hpf-mode`, `hpf-frequency`, `lpf-mode`, `lpf-frequency`, `input-to-sidechain`, `input-to-link`, `sidechain-to-input`, `sidechain-to-link`, `link-to-input`, `link-to-sidechain`; the reverse mode is refused because the gate here has no upward mode, and the hysteresis pair is reported because the reference passes it to the LSP plugin's `ht` and `hz` ports and their exact convention is not verifiable from this checkout |
| `deesser` | `f1-freq`, `threshold`, `ratio`, `mode` | `frequency`, `threshold`, `ratio`, `mode` (`Wide` or `Wideband`) | `detection`, `laxity`, `makeup`, `f1-level`, `f2-*`, `sc-listen`. The reference default mode is `Wide`, so a document without one becomes wideband, as it does in the reference. |
| Auto gain | `target`, `silence-threshold` | `target` (LUFS), `silence_threshold` | `maximum-history`, `reference`, `force-silence`; the reference measures EBU R128 with libebur128 and this auto gain measures EBU R128 with its own implementation over a rolling window |
| `delay` | `time-l`, `time-r`, `mode-l`, `mode-r`, `wet-l`/`wet-r` | `delay_left`, `delay_right`, `mix` | `dry-*`, `sample-*`, `meters-*`, `centimeters-*`, `temperature-*`, `invert-phase-*`; a non-time mode keeps the reference default delay (0 ms) and is reported, and so is a left/right wet pair that differs, because one mix cannot carry both. |
| `reverb` | `predelay`, `decay-time`, `hf-damp`, `diffusion`, `amount`, `dry`, `bass-cut`, `treble-cut` | `pre_delay`, `room_size` (`decay-time` × 40), `damping` (`hf-damp` ÷ 200), `diffusion` (× 100), `bass_cut`, `treble_cut`, `wet`, `dry` | `room-size`; `decay-time` and `room-size` collapse onto one control here |
| `expander` | `threshold`, `ratio`, `knee`, `attack`, `release`, `makeup`, `wet`, `sidechain.mode`, `hpf-mode`, `hpf-frequency`, `lpf-mode`, `lpf-frequency` | `threshold`, `ratio`, `knee` (width, positive), `attack`, `release`, `makeup`, `mix`, `detector`, the two sidechain filters | `dry`, `mode`, `stereo-split`, the sidechain mix controls, the `Low-Pass` and `SMA` sidechain modes, and the upward mode, which the expander here does not implement |
| `maximizer` | `threshold`, `release` | `threshold`, `release` | `input-gain`, `output-gain`; the reference writes only these two, and the maximiser here adds no lookahead |
| `crusher` | `mode`, `bit-reduction`, `morph`, `anti-aliasing`, `sample-reduction`, `lfo-active`, `lfo-range`, `lfo-rate`, `dc` | `mode`, `bits`, `mix` (`morph` × 100), `anti_aliasing` (`anti-aliasing` × 100), `samples`, `lfo_active`, `lfo_depth`, `lfo_rate`, `dc` | `input-gain`, `output-gain`; unknown modes are reported |
| `exciter` | `amount`, `harmonics`, `scope`, `ceil`, `blend`, `ceil-active` | `amount`, `harmonics`, `scope`, `ceil`, `blend`, `ceil_active` | `input-gain`, `output-gain`, and `listen`, which the reference keeps out of the preset |
| `crossfeed` | `fcut`, `feed` | `cutoff`, `feed` | `input-gain`, `output-gain` |
| `crosstalk_canceller` | `delay-us`, `decay-db`, `phantom-center-only` | `delay`, `decay`, `phantom_center` | `input-gain`, `output-gain` |
| `midside_equalizer` | `mid.bandN.{frequency,gain,q,type}`, `side.bandN.{…}`, `output-gain` | `mid_bandN_gain`, `side_bandN_gain`, shared `bandwidth`, `output_gain` | `input-gain`, `num-bands`, `mode`, `decramp`, `split-channels`, `balance`, `pitch-mid`, `pitch-side`; per-band `mode`, `slope` and `width`, and bands that are muted, non-soloed or not bell or shelf |
| `multiband_compressor` | `bandN.{enable-band,split-frequency,compressor-enable,attack-threshold,ratio,knee,attack-time,release-time,makeup}` | `bands`, `splitN`, `bandN_{enabled,threshold,ratio,knee,attack,release,makeup}` | every other global and per-band key (the sidechain block, `compression-mode`, `boost-*`, `stereo-split`, `input-to-*`); a document with more than four enabled bands is reported and the extra bands are dropped |
| `multiband_gate` | `bandN.{enable-band,split-frequency,gate-enable,curve-threshold,reduction,attack-time,release-time,makeup}` | `bands`, `splitN`, `bandN_{enabled,threshold,range,attack,release,makeup}` | the hysteresis block (`curve-zone`, `hysteresis`, `hysteresis-threshold`, `hysteresis-zone`), the sidechain block, `gate-mode`, `stereo-split`, `input-to-*`; a reverse reduction is refused and the band is left closed |
| `voice_suppressor` | `freq-start`, `freq-end`, `correlation`, `phase-difference`, `inverted-mode` | `start`, `end`, `correlation`, `phase`, `inverted` | `maximum-instantaneous-frequency`, `minimum-kurtosis` |
| `rnnoise` | `inputGain`, `outputGain`, `enableVad`, `vadThres`, `wet` | `input_gain`, `output_gain`, `vad`, `vad_threshold`, `mix` (`wet` is dB and becomes a percentage) | `modelName` and `useStandardModel`, because only the standard model is carried here, and `release`, which the reference applies as a grace period before a hard mute while this stage gates with a smoothing constant |
| `pitch` | `semitones`, `octaves`, `cents`, `wet` | `semitones` (`semitones` + 12 × `octaves`, clamped to ±24), `cents`, `mix` (`wet` is dB and becomes a percentage) | `inputGain`, `outputGain`, `dry`, `quickSeek`, `antiAlias`, `sequenceLength`, `seekWindow`, `overlapLength`, `tempoDifference`, `rateDifference`; a combined shift wider than ±24 semitones is reported as clamped |
| any other instance | - | - | the whole instance is named in the warning list |

Two of these conversions are rescalings rather than transfers and are recorded
as such: the reference reverb `decay-time` is a decay in seconds while `room_size`
here is a comb-feedback percentage, and `hf-damp` is a cutoff in hertz while
`damping` here is a percentage. Both are monotonic, and the reference defaults
(`1.5 s`, `5000 Hz`) land on the defaults of this project (`60 %`, `25 %`).

Every converter treats the reference's per-plugin `input-gain` and `output-gain`
the same way. Where this project has an equivalent control, the value is
transferred: the equalizer output gain, the limiter and stereo tools input gain,
the bass enhancer and crystalizer output gain, and both gains of the noise
reduction. Everywhere else both keys are reported. A reference preset that
changes the level of the filter, compressor, gate, de-esser, auto gain, delay or
reverb therefore says so instead of loading at a different level.

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
