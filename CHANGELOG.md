# Changelog

Notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The preset file format carries its own `schema_version`, which is tracked
separately from the application version so that a preset written by one release
keeps working in the next.

## [Unreleased]

### Added

- A partitioned FFT convolver with five built-in responses (*Small room*,
  *Bright hall*, *Dark hall*, *Plate* and *Vocal booth*), wet/dry, width, auto
  gain and a declared 256-frame latency. It is a real uniform overlap-save
  convolution, checked against direct convolution sample by sample; it does not
  load external IRS files or SOFA responses.
- An echo canceller for the input pipeline: a normalised-LMS adaptive filter
  that subtracts the playback leaking into the microphone, with *Echo delay*,
  *Filter length*, *Adaptation*, *Mix* and *Output gain*. The far-end reference
  comes from the output pipeline through a new lock-free stereo bus in the
  engine, so neither pipeline blocks the other. It is not WebRTC and has no delay
  estimation or double-talk detector.
- Spectrum display options: *Shape* (bars, line, filled line), *Resolution*
  (24, 48 or 96 points), *Height* (compact, normal, tall) and *Frequency range*
  (20 Hz to 20 kHz, 10 kHz or 5 kHz), all persisted in the settings document and
  applied to both pipelines alongside the existing decibel range.
- A thirty-band equalizer: the effect covers 20 Hz to 16 kHz in third-octave
  steps instead of ten octave bands, and the editor splits the bands into
  *Lows*, *Mids* and *Highs* so the page stays navigable. The ten original bands
  keep their parameter identifiers and frequencies and the twenty new bands sit
  between them, so a preset saved before the expansion loads with the same curve.
  The mid/side equalizer inherits the same thirty bands. A reference equalizer
  document is folded onto the nearest of the thirty bands instead of the nearest
  of ten, and a test asserts that the conversion's band tables match the effect's.
- Preset autoload per device: *Remember* in Preferences stores the loaded preset
  for the resolved device node in the settings document, *Forget* removes the
  rule, and a pipeline that resolves a different device loads the mapped preset
  on an idle callback. The rule is keyed on the node name; the reference also
  stores and matches the device profile and description. A unit test round-trips
  the rules through the document.
- A *Spectrum range* chooser in Preferences: wide (-72 to +6 dB, the default),
  standard (-60 to 0 dB) or loud (-48 to +12 dB). It is persisted as
  `spectrum_range` in the settings document, applied to both pipelines, and the
  analyser redraws through the selected span; a test asserts every range has a
  positive span and that the value round-trips through the document.
- Eight bands are now available on the multiband compressor and the multiband
  gate, up from four, with seven split frequencies; four remain the default so
  the shipped chain keeps its cost. The crossover now applies the all-pass
  compensation for every band below a split and keeps a separate compensation
  chain per band, because a shared one advanced the same filter twice per sample;
  the eight bands sum within about 0.5 dB. The reference conversion transfers all
  eight bands instead of reporting the ones above four.
- A level bar on every chain row, measured after that stage runs and published
  with the global meters, so the per-stage meters the reference exposes are part
  of the chain instead of a plugin that consumes a slot. The chain records each
  slot's peak, the engine converts it to decibels, and the 50 ms poll writes it to
  the row; a disabled slot and a bypassed chain report no level.
- Tempo sync on the delay: a *Tempo* control of 20–300 BPM and a per-channel
  *Left sync* / *Right sync* division from `1/1` to `1/32`, which replace the
  millisecond time on that channel when it is set. A quarter note at 120 BPM
  lands at 24 000 samples at 48 kHz, which is the new test.
- The reference's six room characters on the reverberation (*Small*, *Medium*,
  *Large*, *Tunnel-like*, *Large/smooth*, *Experimental*), each retuning the comb
  and all-pass lengths and the stereo spread; the reference's `room-size` label is
  converted onto it, and an unknown label stays at *Large* and is reported.
- The voice suppressor's local-kurtosis gate, so *Minimum kurtosis* is a working
  control instead of a reference setting the conversion reported and dropped.
  The kurtosis of the magnitude spectrum over the three bins around each bin is
  compared with the parameter (the reference's 0–100 range, default 1), and a bin
  below it is not treated as voice. A reference `minimum-kurtosis` is transferred,
  the out-of-phase, zero-mix and instantaneous-frequency tests still pass, and two
  new tests pin that a threshold above the tone's local kurtosis releases it and
  that a flat magnitude triplet reads below a peaked one.
- A loudness meter on the output meter card: the EBU R128 short-term loudness of
  the processed audio in LUFS, measured by the same `Loudness` implementation the
  auto gain uses, published through atomics and refreshed with the meters. The
  momentary and integrated values are measured and published too. The readout
  shows `-inf LUFS` until three seconds of audio have been measured. A unit test
  round-trips the published values, and a processor test pushes a half-scale tone
  through the real ring and asserts a finite short-term value between −30 and
  0 LUFS.
- The CPU baseline times the loudness measurement next to the spectrum analyser,
  so the cost of the new meter in the audio path has a reproducible figure.

- A *Loudness* compensator, the twenty-seventh effect: an equal-loudness contour
  with the reference's five standards (Flat, ISO 226:2003, Fletcher-Munson,
  Robinson-Dadson, ISO 226:2023), an output volume, and hard clipping with a
  clipping range. Its contour follows the output volume, so a full volume is flat
  and a quiet one lifts the bass and the treble. `crates/dsp/src/analysis/r128/mod.rs` also
  gained the momentary and short-term windows the reference reports.
- The crystalizer now splits thirteen bands, each with its own intensity in
  decibels, a *Mute* and a *Bypass*, matching the reference's `band0`...`band12`
  vocabulary; its reference conversion transfers all thirteen bands instead of
  reporting them.
- A sidechain high-pass and low-pass on the gate, of 12, 24 or 36 dB/oct, so
  rumble or hiss can be kept out of the detector; both are off by default, and
  the reference's `hpf-*` and `lpf-*` keys are transferred.
- An *Autotune*, the twenty-ninth effect: a normalised autocorrelation finds the
  pitch, the nearest semitone against a reference gives a target, and the shared
  phase vocoder applies the correction at a chosen speed and strength.
- A *Harmonizer*, the thirtieth effect: two phase-vocoder voices with their own
  semitone interval and level, summed with the dry signal.
- A *Speech processor*, the twenty-eighth effect: a noise floor tracked with a
  fast-fall/slow-rise envelope, voice activity from the level above that floor,
  a downward noise reduction proportional to how far the signal is from voice,
  and an automatic gain driven by the R128 short-term loudness. Its reference
  conversion transfers the stage switches and the suppression depth, and reports
  the VAD probabilities, the dereverb switch and the per-plugin gains.
- The instantaneous-frequency gate on the voice suppressor now decides whether a
  bin counts as voice, instead of the setting being accepted and dropped.

- A pitch shift effect, the twenty-fifth, and a bass loudness effect, the
  twenty-sixth. Pitch shift is a phase vocoder:
  `crates/dsp/src/effects/pitch_shift/mod.rs` runs a 2048-point Hann-windowed STFT at
  75 % overlap, recovers each bin's instantaneous frequency from the phase
  advance, remaps the bins by the shift ratio and propagates the synthesis
  phases, then sums the frames back by overlap-add. It exposes *Semitones*
  (±24), *Cents* (±100) and a dry/wet *Mix*, and declares the 2048-frame analysis
  window as its latency, which the Diagnostics dialog already reports. Its tests
  measure the shift: an octave up turns a 1 kHz tone into 2 kHz, an octave down
  turns 2 kHz into 1 kHz, a zero shift leaves the frequency alone, +100 cents
  reaches 1059 Hz, an impulse lands at the declared latency, a zero mix is
  bit-identical to the input, and the output stays finite at the widest settings.
  Bass loudness is an equal-loudness contour built from a low shelf at 120 Hz and
  a high shelf at 6 kHz whose gains grow as the source level falls, plus the
  static *Output* trim and the *Link* compensation; its tests measure that a quiet
  source lifts 60 Hz more than 3 dB above the midrange, a full-level source leaves
  the contour flat within 0.6 dB, the output trim lands on its decibels, and the
  link setting lowers the output as the source falls.
- `ParamUnit::Semitones` and `ParamUnit::Cents`, so the pitch shifter's two shift
  controls show `st` and `ct` in the editor instead of a bare number.
- Reference conversion for the `pitch` and `bass_loudness` blocks:
  `semitones` + 12 × `octaves` and `cents` are transferred to the shifter (a shift
  wider than ±24 semitones is reported as clamped), `wet` becomes the mix, and
  the SoundTouch mode settings have no equivalent and are reported. The three MDA
  *Loudness* controls are transferred as they are.
- A pitch shift and a bass loudness entry in the effect picker, each with its own
  symbolic icon, and the two icons are compiled into the binary and checked by
  the icon tests like every other effect.

- The full Calf Stereo Tools control set on *Stereo tools*: all seven routing
  modes (stereo, stereo to mid/side, mid/side to stereo, mono left, mono right,
  mono sum and swap), input and output balance, per-channel phase and mute,
  middle and side level, middle panorama, side balance, width, a Haas delay and
  soft clip. The reference's `side-level` and `middle-level` are transferred in
  decibels now instead of being folded onto the width, and its `stereo-phase` and
  `sc-level` are reported rather than approximated.
- A sidechain high-pass and low-pass on the compressor and the expander, of 12,
  24 or 36 dB/oct, so rumble or hiss can be kept out of the detector. Both stages
  are off by default, which is what keeps a preset written before this change
  sounding the same. The reference's `hpf-*` and `lpf-*` keys are transferred.
- Diffusion and a tone stage on the reverberation: `diffusion` sets the all-pass
  feedback, and `bass_cut`/`treble_cut` are a high-pass and a low-pass on the wet
  signal, applied through the same biquad design the rest of the DSP uses. The
  reference's `diffusion`, `bass-cut` and `treble-cut` are transferred.
- A real lookahead on the limiter, which is what makes it a limiter rather than a
  clipper: the audio is delayed by the lookahead, the gain envelope descends
  linearly across that window so a transient is attenuated before it reaches the
  output, and a per-sample requirement still clamps the result so the ceiling
  holds by construction. `lookahead` (0.1–20 ms) and `stereo_link` (0–100 %) are
  new parameters, both transferred from a reference preset; a full-scale burst now
  keeps its sine shape instead of being clipped, which is asserted as a crest
  factor within 0.03 of √2.
- A reported chain latency. `Effect::latency_frames` lets a stage declare what it
  delays, the chain sums the enabled slots, the processor publishes the total, and
  the Diagnostics dialog shows it in frames and in milliseconds next to the
  quantum. Nothing compensates for it, and the documentation says so: the two
  streams are joined by a ring buffer rather than by graph links, so there is no
  graph latency to adjust.
- A curve and hysteresis on the gate. `curve` is the zone below the threshold over
  which the reduction ramps instead of switching, and `hysteresis` is how far
  below the open threshold the gate closes, so a signal near the threshold no
  longer chatters. The reference's `curve-zone` is transferred into `curve`; its
  `hysteresis-threshold`/`hysteresis-zone` pair is reported rather than guessed
  at, because the reference passes those to the LSP plugin's ports and their exact
  convention cannot be verified from this checkout.
- Five more built-in presets beside *Default*: *Podcast voice*, *Music,
  speakers*, *Headphones*, *Bass boost* and *Clean microphone* (the last one for
  the input pipeline). `make install` writes all six into the system preset
  directory under their preset names, and the presets crate now dev-depends on the
  effect registry so a test can assert that every built-in parses, names only
  effects that exist, and sets only parameters those effects declare.

- A noise reduction effect, the twenty-fourth, backed by
  [`nnnoiseless`](https://crates.io/crates/nnnoiseless), the Rust port of RNNoise:
  input and output gain, a dry/wet mix, and a voice-activity gate with its
  threshold. It processes only at 48 kHz, the rate RNNoise is defined for; at any
  other graph rate the stage passes the signal through. `nnnoiseless` keeps its
  window and DCT tables in a process-wide cell that is filled on first use, so the
  effect runs one frame of silence through a throwaway state when it is built, on
  the control thread, rather than letting the audio callback pay for it. It maps
  the reference's `rnnoise` preset block, transferring both gains, the gate, its
  threshold and the wet level, and reporting the model settings and the release
  grace period it has no control for.
- A live spectrum under the meters: 48 logarithmic bands from 20 Hz to 20 kHz,
  measured from the mid signal with an 8192-point Hann-windowed transform every
  third block, published through atomics and drawn with a band grid and the
  loudest band's frequency and level as a caption. Each band is the integral of
  the Parseval-corrected bin power between its edges, normalised to the analysis
  bandwidth when the band is narrower than the window's resolution, so the display
  sums the tones inside a band and a full-scale tone reads 0 dB like the meters
  beside it. The analyser is built with the processor, so nothing is allocated in
  the callback, and it costs 117 μs per 1024-frame block, 0.55 % of one core. A
  pipeline whose engine has stopped clears its snapshot, so the display never
  keeps the last frame it happened to measure.
- `nnnoiseless` 0.5.2 as a dependency of `pwe-dsp`, and with it `easyfft`,
  `realfft`, `rustfft` and `once_cell`. All are MIT or Apache-2.0 except anymap3
  (BlueOak-1.0.0 or MIT or Apache-2.0) and the crate itself (BSD-3-Clause); every
  one is compatible with this project's GPL-3.0-or-later. Credited in
  `CREDITS.md`.
- The noise reduction effect has its own live test:
  `crates/engine/tests/live_noise_reduction.rs` runs it in the real chain on a
  private sink and asserts that a −20 dB tone arrives at its own level and leaves
  more than 20 dB lower, with the underruns below an eighth of the processed
  frames. It samples the meters across the run and keeps the largest reduction it
  sees: three consecutive runs measured the tone leaving 124.00 dB, 96.14 dB and
  50.52 dB lower, each over 12 544 to 25 600 processed frames with 512 underrun
  samples from the initial fill and no overruns or trims, which is what shows the
  dependency's lazily planned transform does not break the audio callback. Reading
  the meter once, as the test used to, made it fail intermittently on a loaded
  machine.
- The CPU baseline's table in `docs/architecture.md` gained the limiter and the
  gate, whose cost is no longer below the timer's resolution: the lookahead costs
  0.041 ms per 1024-frame block and the gate's curve 0.026 ms.
- The live GPU tests can fail on a loaded machine, and that is now written down
  next to their evidence rather than left to be rediscovered. The persistent GPU
  stage costs 302.5 μs for a 1024-frame block against a 10.7 ms fence budget, so a
  fallback means the scheduler stalled, not that the stage is slow: the same tests
  measured 0 fallbacks on a quiet machine and 11–17 % with a Bluetooth default
  output and a load average of 3–6, with zero disagreements throughout because a
  missed deadline applies the gain on the CPU.
- The CPU baseline now times the spectrum analyser, because it runs in the audio
  path, and reports the fastest of three rounds per workload instead of a single
  run, so concurrent desktop load cannot inflate a figure.
- The analyser's tests generate their signals with the phase carried across
  blocks. The earlier helper restarted the wave every block, which made the test
  audio a periodic signal with a 1024-sample period instead of a tone; two of the
  new level assertions passed by luck until it was fixed.

- A multiband compressor, a multiband gate and a voice suppressor, and the two
  primitives they need: a hand-written iterative radix-2 FFT
  (`crates/dsp/src/primitives/fft/mod.rs`) and a Linkwitz-Riley crossover
  (`crates/dsp/src/effects/crossover/mod.rs`). The multiband effects split the
  spectrum into up to four bands; the voice suppressor attenuates centred voice
  from the inter-channel correlation and phase difference in the frequency
  domain. Each maps its EasyEffects preset block and is marked *Partial* in
  `docs/feature-matrix.md`, which records what differs.
- Seven more effects: an expander, a maximizer, a bit and sample-rate crusher, an
  exciter, a headphone crossfeed, a cross-talk canceller and a mid/side
  equalizer. Each has its own symbolic icon, its own parameter set and its own
  tests, and each maps a matching EasyEffects preset block. The cross-talk
  canceller is a port of the reference's own native algorithm; the others are
  native Rust designs against the reference's parameter sets and are marked
  *Partial* in `docs/feature-matrix.md`, which records what differs.
- The effect registry now checks itself: identifiers and labels must be unique,
  every effect must publish parameters and build, and every parameter identifier
  must be unique within an effect.
- `ParamUnit::Microseconds`, for the cross-talk canceller's delay.
- The slider rows carry an accessible label and value, so an assistive
  technology announces a parameter with its unit and its current value.
- The application icon was redrawn as a hand-written vector: the same mark as
  before - a gradient ring around five level bars - rebuilt with a thicker ring,
  a full-bleed rounded tile and soft background washes, so it stays legible down
  to 32 px, follows the shell's corner mask and matches the usual GNOME look.
- The input (microphone) pipeline. It captures from the chosen input device,
  runs a second effect chain and publishes the result as the virtual
  `pipewire-effects-source`, which applications can record from. The Effects page
  gained an *Output* / *Input* switch, the preset format gained an `input`
  section, and the EasyEffects importer converts both sections. The pipeline is
  opt-in and starts only when *Process microphone input* is switched on.
- A settings file, `$XDG_CONFIG_HOME/pipewire-effects/settings.json`, which
  remembers the destination device of each pipeline, the global bypass, the
  loaded preset, whether microphone processing runs, and whether the application
  autostarts. It is written atomically and a damaged file falls back to the
  defaults with a message.
- A command line: `--help`, `--version`, `--preset NAME`, `--device NAME`,
  `--no-window`, `--list-presets` and `--list-devices`. `--no-window` starts the
  processing without a window and opens the interface on the next launch.
- An *Start on login* switch that writes and removes
  `$XDG_CONFIG_HOME/autostart/io.github.D1ZZY4.PipewireEffects.desktop`.
- A *Reconnect* button on the Devices page, shown when the engine has failed.
- `make install`, a `PKGBUILD` and a Flatpak manifest.
- Internationalization. Every user-facing string is translated through gettext,
  with a catalog in `po/` (`POTFILES.in`, `LINGUAS` and a generated
  `pipewire-effects.pot`) and `scripts/update-po.sh` to keep it current. Effect
  labels, parameter labels, groups and choice labels are translated where they
  are read, so a new effect needs no interface change. `make install` compiles
  the catalogs into `$(PREFIX)/share/locale`. No translation ships yet, so an
  installed build shows English until one is added.
- `scripts/capture-ui.sh`, which renders every interface state on an isolated
  headless GNOME session and writes one PNG per state, and `scripts/update-po.sh`.
  See `scripts/README.md`.
- `crates/dsp/tests/benchmark.rs`, a reproducible CPU baseline. It times each
  effect and the full twenty-three-effect chain and prints microseconds per block, the
  realtime factor and the cost as a percentage of one core, at a 1024-frame block,
  and fails if the whole chain ever needs more than half of the block.
  `cargo test --release -p pwe-dsp --test benchmark -- --ignored --nocapture`.
- The window's navigation is an *Output* / *Input* switcher in the header. Each
  pipeline owns its own sidebar, its own *In* and *Out* meters and its own
  editor, so a chain is never drawn against another pipeline's levels. The effect
  chain, the on/off *Effects* switch and the preset library share the left
  sidebar; a header toggle hides it, and below 860 px it becomes a drawer. The
  window's minimum size is 360x480.
- *Devices* and *Preferences* moved into the main menu and open
  `adw::PreferencesDialog`s. The menu entries carry icons and have `Ctrl`+`4`
  and `Ctrl`+`,` accelerators.
- A *Developer mode* switch in Preferences. The engine counters and the
  diagnostic messages are shown only while it is on, and the choice survives a
  restart in the settings file.
- `ParamUnit::Lufs`, and an EBU R128 loudness measurement in
  `crates/dsp/src/analysis/r128/mod.rs`: ITU-R BS.1770 K-weighting for the running sample
  rate, 400 ms blocks at a 100 ms hop, the absolute −70 LUFS and relative −10 LU
  gates, and the −0.691 offset.
- A preset can be unloaded and duplicated. The check button on a preset row is
  accented for the loaded preset and unloads it, which clears the chain and
  leaves no preset active; a *Clone* button in the preset actions duplicates the
  loaded preset, both sections, under a new name in the user directory.
- *Auto-save preset edits* and an *Update interval* in Preferences. When it is
  on, an edit to the loaded preset is written back to the preset after the
  interval has passed without further changes. The loaded preset's row always
  offers *Update preset* and *Revert changes* from its own menu.
- A selectable processing backend in *Preferences*: `CPU`, `GPU` or `Both`.
  `GPU` runs the elementwise gain stage on a Vulkan compute device through
  `crates/engine/src/gpu` (the `ash` crate, `scale.comp` compiled to
  `scale.spv`), while recursive and spectral effects stay on the CPU. `Both`
  adds a per-block CPU reference and replaces a result that disagrees by more
  than `max(1e-5, 1e-5 · |reference|)`. The choice is stored as `audio_backend`,
  applies to both pipelines, and restarts the running engines when changed. The
  instance, device, buffers, pipeline and a warm-up dispatch are prepared before
  the audio thread starts. Per block the fence wait is capped at half the block
  duration (100 µs floor, 25 ms ceiling); a miss applies the gain on the CPU and
  increments `GPU fallbacks`, and three consecutive misses or a Vulkan error
  latch the accelerator off for that engine. Six unit tests cover the probe,
  the persistent scaler, the compute path, backend parsing and the accelerator,
  and two ignored live tests run `GPU` and `Both` against a real daemon and a
  private sink. The ignored timing test measured one 1024-frame stereo block at
  0.17–0.22 µs on the CPU and 140–330 µs through the persistent GPU stage
  depending on load, so the setting is a real backend choice, not a performance
  claim. The built-in default is `Both`, so a fresh installation off-loads the
  gain stage and verifies it against the CPU; a machine without a Vulkan device
  reports a warning and keeps the CPU path.
- The engine counters, the detected Vulkan compute devices and the diagnostic
  messages moved out of *Preferences* into a *Diagnostics* dialog (`Ctrl`+`5`).
  *Developer mode* now adds a *Diagnostics* entry to the main menu, and the
  entry is absent while the switch is off.

### Fixed

- The effect glyphs in the Add-effect picker, the chain and the editors were
  painted as solid silhouettes or disappeared entirely: *Multiband compressor*,
  *Multiband gate* and *Auto gain* were blank, while *Expander*, *Maximizer*,
  *De-esser* and *Loudness* showed filled blobs. GTK 4.22 and later render a
  `-symbolic.svg` in traditional symbolic mode and derive fill and stroke from
  the symbolic CSS class, ignoring the standard `fill="none"` and `stroke`
  attributes, so every stroke-only path became a filled outline region. All
  drawing elements now carry `foreground-fill` or
  `transparent-fill foreground-stroke`, and the icon test asserts the classes.
  The picker was captured with every category visible in both appearances
  (`target/ui-verify-icons`).
- The effect picker scrolled its list directly under the search field, and its
  overlay scrollbar was invisible until it was hovered, so a list scrolled to the
  middle showed a row cut in half against the search box with no sign that more
  content sat above it. A separator now marks the boundary, the scrollbar is
  always drawn, and changing the filter resets the scroll to the top so the first
  match is visible.
- The per-stage level bar added to every chain row took 40 px from the effect
  name, so at the 1040 px window *Bass enhancer*, *Reverberation* and *Stereo
  tools* were drawn as `Bass en…`, `Reverbe…` and `Stereo t…`. The bar moved
  under the name as a 3 px meter, which gives the label the width it had before
  the bar existed, and it is hidden while the stage is digitally silent so an
  idle chain has no line under every name. The captures in `target/ui-verify-fix`
  show every effect name in full.
- The mid/side reference conversion wrote each folded band by position, which
  named the wrong frequency once the equalizer's band identifiers stopped being
  in frequency order. It now derives the `mid_`/`side_` identifier from the
  equalizer's own band table, so a converted mid/side band lands on the band it
  names.
- The multiband split defaults were 80, 200 and 500 Hz for a four-band chain,
  which put all three crossovers in the bass and left 500 Hz to 20 kHz as one
  band. The first three splits default to 120, 800 and 5 000 Hz again, the values
  the four-band effect used before the eight-band change, with four more splits
  above them for five to eight bands. A test pins the four-band values so a later
  change cannot move them silently.
- The multiband converter reported its own `bandN` keys as "band* was left at
  its default" even though it consumed them, because it used the plain `skipped`
  helper instead of `skipped_with_bands`. A reference multiband preset now loads
  without that spurious warning, and the test that asserts a clean eight-band
  load catches it.
- `crates/ui/src/views/meter/mod.rs` was missing from `po/POTFILES.in`. The file carried no
  message of its own before the loudness meter, so nothing was visibly missing,
  but its new tooltip string was dropped from the template. The file is listed
  now and the string is extracted.
- Two source files with user-facing messages were missing from
  `po/POTFILES.in` and their strings were absent from the template:
  `crates/presets/src/ee/params/speech/mod.rs` (the `vad.*` warning) and
  `crates/ui/src/window/autoload/mod.rs` (the autoload-warnings banner). Both are
  listed now.
- Five label tables were not wrapped in `pwe_core::i18n::mark`, so their strings
  never reached the template: the stereo-tools routing modes (pre-existing), the
  reverb characters, the delay tempo divisions, the multiband split names, and
  the crystalizer's `Low`/`Mid`/`High` groups. The mid/side equalizer's `Mid
  bands` and `Side bands` group names were also plain literals passed through a
  helper. All are marked now, and `scripts/update-po.sh` writes a 578-message
  template from 84 files.
- A regression test now guards the catalog: `registry::tests::
  the_translation_template_carries_every_label_the_registry_publishes` reads the
  committed template and fails if any effect label, summary, parameter label,
  parameter group or choice label the registry publishes is missing from it. The
  earlier claim that no omission existed beyond `meter.rs` was wrong; this test
  is the check that would have caught all of the above.
- `docs/feature-matrix.md` never listed five of the effects the registry
  implements: the mid/side equalizer, the maximizer, the crossfeed, the crosstalk
  canceller and the exciter. Each has a row now with its reference evidence,
  parameters and tests, so every registered effect is traceable from the matrix.
- `docs/feature-matrix.md` still said the voice suppressor's
  `maximum-instantaneous-frequency` and `minimum-kurtosis` were reported rather
  than applied, which stopped being true when the frequency gate was added and is
  now false for the kurtosis gate too. Both keys are recorded as transferred.
- `docs/reference-audit.md` still said autotune "needs a pitch-detection stage and
  is not written", which stopped being true when the effect was added. The
  dependency note now describes the vocoder the three pitch effects share.
- `docs/architecture.md` described the two meters as `gtk::LevelBar` widgets. They
  are `gtk::DrawingArea` cards that draw their own segments, RMS marker and hold,
  and the section now says so.

- Stereo tools reports the reference's `dry` and `wet` instead of listing them
  as handled and dropping them. Both keys were in the effect's handled list but
  neither was transferred, so a reference preset that sets them loaded at a
  different mix in silence. They are reported now, which is what
  `docs/feature-matrix.md` already claimed.
- `docs/architecture.md` claimed the limiter had no lookahead, which stopped
  being true when the lookahead was added. The trade-off list now records what
  the two delaying stages actually cost: the limiter's lookahead and the
  pitch shifter's analysis window are both reported and neither is compensated.

- The spectrum no longer reports a level that belongs to a different frequency.
  With a 1024-point transform at 48 kHz the bin width is 46.9 Hz, so 22 of the 48
  bands sat below the analysis resolution and nine of them reported one shared
  bin under nine different frequency labels; the caption could name 21 Hz for
  content at 47 Hz. The transform now has the reference analyser's own default
  size of 8192 points, a band is the integral of the bin power between its edges
  rather than one peak bin, and a band narrower than the window's resolution is
  normalised to it, which is what the reference does.
- A reference RNNoise preset no longer loads at the wrong wet level. The `wet`
  key is in decibels and has an equivalent control here, so it is transferred to
  the mix like the gate's and the expander's, and the `modelName`,
  `useStandardModel` and `release` keys, which have no control here, are reported
  instead of being listed as handled and dropped silently.
- Every symbolic icon file now keeps each SVG tag on one line, which is the
  convention the rest of the palette already followed; the interface test reads
  every effect's icon file and fails on one that is missing, that does not draw
  from `currentColor`, that is not a 16 px symbolic icon, or that leaves a tag
  open across lines. The line-break rule is cosmetic: the two spellings were
  measured to render identically through both `rsvg-convert` and the gdk-pixbuf
  SVG loader, so this is a formatting change and not a rendering fix.
- Changing, saving or deleting a preset no longer closes the application. The
  three preset handlers matched on the result of a `State` mutation while the
  `RefCell` borrow was still alive (`match ui.state.borrow_mut() …`), and the
  matched arm re-entered the interface, which borrows the same cell; the release
  profile is compiled with `panic = "abort"`, so the first preset action aborted
  the process. The result is bound to a local before it is matched, which drops
  the borrow before the interface is touched, and the same ordering is applied to
  the other handlers that react to a state mutation.
- The screenshot harness no longer races the first frame of a dialog. Capturing a
  state that opens a dialog (the About window, the effect picker) occasionally
  found no render node and produced no PNG; the capture now retries until the
  render tree is ready.
- The destination device now follows the system default sink. The engine reads
  `default.audio.sink` from the session manager's metadata before it creates its
  own sink, and re-resolves the destination when that value changes while it
  runs. Previously the output was chosen by a `priority.session` heuristic, so on
  a machine whose default is a Bluetooth headset the processed audio was sent to
  the built-in analogue output and nothing was heard. The *Devices* page gained a
  *System default* entry so the automatic choice can be restored after picking a
  device by hand.
- The input and output level meters now measure with the same ballistics. The
  output meter computed the raw block peak while the input meter ran a decaying
  peak hold, so an effect-free chain reported different levels on the two sides
  (a live −12 dBFS signal read as −26 dBFS at the output). Both meters share
  `LevelMeter` now, and a test asserts that an empty chain reports the same level
  at both.
- The application icon resolves when the program is run from a build directory.
  The icon is compiled into the binary as a `GResource` and registered with the
  icon theme, so the About dialog and the header bar no longer show the
  missing-image glyph on a machine where `data/icons` was never installed.
- Every icon the interface uses exists in the Adwaita icon theme, and the set is
  checked at startup and in a test. `emblem-music-symbolic` (the tone effects and
  the *Effects* page) and `utilities-system-monitor-symbolic` (the analysis
  category) are not part of Adwaita, so they were drawn from an unrelated theme
  or replaced by the missing-image glyph. Each effect now has its own icon.
- The effect chain column no longer grows with the length of an effect summary.
  The two columns are an `adw::OverlaySplitView` with a fixed sidebar width, so
  the editor keeps enough room for its controls: a parameter labelled *Release*
  no longer wraps in the middle of the word.
- A window narrower than 860 px collapses the chain into an overlay with a header
  toggle instead of squeezing both columns.
- A device node that leaves the graph is now removed from the discovered device
  list; the registry listener handles `global_remove`.
- The virtual sink no longer inherits a volume or a link target remembered by
  the session manager. Both internal streams opt out of the session manager's
  restore hooks, so the processing path always runs at unity gain. A stored
  volume for the sink name previously attenuated everything entering the chain:
  a −20 dB tone was measured at −55 dB at the input.
- EasyEffects preset conversion now honours the reference defaults. An omitted
  filter slope is order 1 (`x1`), an omitted de-esser mode is wideband (`Wide`),
  and an omitted delay time is 0 ms; these were previously replaced by the middle
  of the range here.
- EasyEffects conversion no longer drops content silently. The two ladder filter
  types are mapped to their closest shape instead of low pass, an unsupported
  compressor sidechain mode is reported, the equalizer folds *both* channel
  blocks into the shared curve and reports a right channel that differs, the
  crystalizer's per-band settings are reported instead of being filtered out of
  the warning list, and a delay whose left and right wet levels differ is
  reported because one mix cannot carry both.
- Preset conversion warnings now summarise a numbered family of settings (for
  example `band*`) instead of listing every band of a crystalizer preset.
- A reference preset's per-plugin `input-gain` and `output-gain` are no longer
  dropped without a word. They are transferred where an equivalent control
  exists (equalizer, limiter input, stereo tools input, bass enhancer and
  crystalizer output) and reported everywhere else.
- The equalizer conversion no longer turns a muted band into an audible one, and
  a soloed band excludes the others as it does in the reference. Bands without a
  usable frequency, per-band `mode`/`slope`/`width`, and a right channel that
  differs only in `mute` or `solo` are all reported.
- A reference document whose parameter block is keyed without the `#<instance>`
  suffix, which the reference still reads, is now found here too.
- A preset file whose name is not a valid preset name is no longer offered in
  the browser, and a malformed file no longer makes the browser and the loader
  disagree about which preset a name refers to: the malformed file is reported
  and the entry that is shown is the entry that loads.
- The preset browser and the loader agree on precedence. A file in a system
  preset directory shadows a compiled-in preset in both, so the entry that is
  shown is the entry that is loaded, and only files in the user directory are
  offered for deletion.
- EasyEffects preset conversion now reads the keys and labels the reference
  actually writes. Shelf filter types, the wideband de-esser mode, the delay
  times, the gate curve threshold and reduction, the auto gain silence threshold
  and the stereo tools side level were previously ignored and silently replaced
  by defaults; the mix level of a fully dry effect was inverted.
- EasyEffects conversion warnings and storage warnings now reach the interface
  instead of being discarded while loading or importing a preset.
- The De-esser reads its detector frequency, attack and release while it runs.
  The band-pass coefficients and the ballistics were computed once, on the first
  block, so moving *Sibilance*, *Attack* or *Release* changed nothing until the
  effect was recreated.
- The reverberation pre-delay line keeps a separate history per channel. One
  line was shared by both channels, so the left channel's pre-delayed path
  carried the right channel's past samples.
- The equalizer importer reports a reference band that falls outside the ten
  fixed bands instead of folding it to nothing in silence, and the mid/side
  importer takes the shared bandwidth from the side block when the mid block
  stores no quality.
- The expander importer reports a reference `dry` setting instead of listing it
  as handled and dropping it.
- The translation template carries the effect, parameter and choice vocabulary.
  Those labels are read through gettext but declared outside a `t()` call, so
  `xgettext` never saw them and a translator had nothing to translate; they are
  marked for extraction and `scripts/update-po.sh` now teaches `xgettext` the
  parameter builders, which grew the catalog from 199 to 348 messages. Effect
  names, parameter labels, groups and choice labels are therefore translatable
  for the first time.
- Two dialog *Cancel* buttons were not translated, and the error banner had no
  way to be dismissed by hand: a handler was wired to its button but the button
  was never shown.
- The level meter's hold and RMS markers and the equalizer's band markers were
  drawn in fixed colours, so the hold and RMS markers were invisible in one
  appearance and the band markers were invisible in the light appearance. They
  are drawn from the widget's own foreground colour now.
- The crystalizer's four band switches were labelled with their parameter
  identifiers (`band1`…`band4`). They read `Below 200 Hz`, `200 Hz to 2 kHz`,
  `2 kHz to 8 kHz` and `Above 8 kHz` now, after the fixed crossovers, and the
  labels go through gettext with the rest of the vocabulary.
- Three EasyEffects conversion warnings were written as bare string literals
  rather than through gettext, so they were untranslatable: the two ladder filter
  mappings and the delay's left/right wet mismatch.
- The effect that the editor is showing is highlighted in the chain again.
  Rebuilding the chain list recreated every row and dropped the selection, so the
  row lost its highlight after a preset load, a move, or an enable toggle, and the
  editor and the chain disagreed about which effect was selected. The selection is
  restored from the chain model whenever the rows are rebuilt or the editor is
  refreshed.
- The crusher's dry path no longer carries the held sample. With a partial mix
  the output was `held * (1 - mix) + crushed * mix`, so the dry term was the
  sample-and-hold value instead of the input sample; a signal that did not reach
  a hold boundary returned the previous held sample. The dry term is the input
  sample now.
- A removed effect is never freed on the audio thread. When the recycle queue was
  full the audio thread dropped the box, which called the allocator inside the
  realtime callback; the processor holds the effect in a small side buffer and
  moves it to the queue on the next block instead.
- The application's own options reach the application again. `Application::run`
  handed the whole process command line to GLib, which rejects an option it does
  not know, so `--no-window`, `--preset` and `--device` printed
  `Unknown option ...` and exited with status 1 instead of starting. The autostart
  entry, which is `pipewire-effects --no-window`, therefore did nothing at
  sign-in. The application is now run with only its program name, and the options
  are read by the parser in `crates/app/src/cli/mod.rs`.

### Changed

- The `pwe-settings` crate is renamed `pwe-services`; it still owns the settings
  document, its atomic store and the autostart entry.
- The save action in the preset row is a flat button with an accent-coloured
  glyph instead of a filled rounded `suggested-action` button, so it matches the
  flat import, export, clone and delete actions beside it.
- The effect chain menu button is drawn on every row instead of appearing only
  while the pointer is over the row, so *Move up*, *Move down* and *Remove* are
  reachable without hovering. A separator now divides the effect chain from the
  preset area in the sidebar.
- The *Presets* heading is gone. The preset actions row (save, import, export,
  delete, clone) now sits above the preset list, under the divider from the
  effect chain, and each preset row carries a round check button that loads or
  unloads that preset.
- Every preset row shows its name in bold with its origin (`Built in`,
  `User preset`, `System preset`) on the same line. The *Delete preset* dialog is
  sized like the other dialogs instead of hugging its content.
- The *Devices* dialog no longer shows a *State* row. It repeated the pipeline
  state that the chain's *Effects* toggle and the switcher already carry; the
  *Format* and *Quantum* rows stay, and the *Reconnect* button, which used to
  live on the removed row, now sits on the *Format* row and still appears only
  when the engine has failed.
- The voice suppressor's icon was redrawn. The previous mark put five strokes
  about 2.5 px apart inside a 16 x 16 box, so at the size the chain rows and the
  effect picker actually draw it (16 px) the strokes merged into an unreadable
  blob. The new mark is a prohibition ring with a diagonal bar, drawn with fills
  so that a symbolic icon recolours it correctly.
- The interface shell was consolidated. The preset library moved from a page of
  its own into each pipeline's sidebar, the chain on/off *Effects* toggle moved
  from the header into that sidebar, and the sidebar is shown or hidden with a
  `sidebar-show-symbolic` toggle in the header. The split view's 1 px separator
  is switched off in the application stylesheet, so the sidebar keeps the theme's
  tinted background without a drawn line.
- The chain rows are a flat list in processing order: no category headers and no
  position numbers, so *Move up* and *Move down* always move an effect past the
  neighbour that is drawn above or below it. Each row carries an on/off switch
  for that effect; the effect icon is plain and the switch carries the accent. The
  effect picker still groups its rows by category.
- The layout is adaptive. Below 860 px the split view collapses, the header
  switcher drops to its icon-only policy and the sidebar becomes a drawer with a
  back button; the window's minimum size is 360x480, and a float row keeps its
  label on one line so a phone-width window ellipsizes it instead of breaking it.
- Auto gain normalises to an EBU R128 loudness target instead of a raw RMS
  level, so its *Target loudness* and *Silence threshold* are in LUFS and a
  reference preset's `target` now means what it means in EasyEffects. The
  measurement covers a rolling 30-second window rather than the whole program.
- The settings document gained `developer_mode`, and the diagnostics that used to
  sit in an *Advanced* group on the Devices page moved into Preferences, behind
  that switch.
- The destination chooser's row was shortened to *Output device* / *Input
  device*, so its value is no longer ellipsized in the narrower dialog.
- The input and output meters sit side by side below the chain header instead of
  one above the chain and one above the editor, and both expand to the width of
  the column.
- The level history trace is not drawn while the meter is silent.
- The diagnostics expander reports overrun samples and the current ring
  occupancy next to the captured frames, processed frames, underrun samples and
  drift trims, so every counter the engine keeps is visible.
- The destination chooser rebuilds its entries whenever the discovered devices
  change, including when a pipeline currently has no matching device, so it can
  no longer show a list that the engine has outgrown.
- A preset named on the command line with `--preset` now reports the conversion
  warnings it produced instead of loading in silence.
- The interface-review harness renders a state through the window's own render
  tree (`PIPEWIRE_EFFECTS_UI_CAPTURE`) and writes it to a PNG, so a capture can
  be taken on a Wayland session, where GNOME refuses its screenshot interface to
  other clients. The `narrow` state sets the window size before the window is
  mapped instead of after it, so it now actually exercises the narrow layout.
- The engine retries by itself. When a stream fails, the streams are rebuilt
  with an increasing backoff instead of leaving the pipeline failed until the
  application is restarted.
- The Devices page follows the pipeline that is being edited, and reports which
  one the counters and the destination belong to.
- The effect chain and the editor are laid out with `adw::OverlaySplitView`, and
  each pipeline is reachable with the `Ctrl`+`1` and `Ctrl`+`2` accelerators.
- The chain sidebar is quieter: each row carries the effect's own symbolic icon
  and an enable switch, the reorder and remove actions live behind a menu button
  drawn on every row, and one plain sentence reports the engine state. The earlier
  summary line and coloured status dot were removed because they duplicated the
  rows and did not say what they measured.
- The live PipeWire processing test opens its own virtual sink under a
  process-specific name, so the level it measures is the tone it played rather
  than whatever the session manager has routed to the shared sink.
- The reference conversion is split per effect family, and every reference
  setting that has no equivalent here is reported rather than dropped.
- The equalizer conversion folds the reference bands using the reference band
  quality as the shared bandwidth.
- The effect registry carries one table. The identifier list and the function
  that returned an effect's parameter specifications were separate and could
  drift apart; each descriptor now carries its specifications, and a descriptor
  that is added without them no longer publishes an empty parameter set in
  silence.
- The compressor and the expander share one detector and one gain stage instead
  of two copies of the same code.
- The playback buffer is bounded by the processing scratch buffer, so a graph
  quantum larger than the application's maximum cannot index past it.
- Unused helpers were removed: the meter's unused reset and loudness accessors,
  the buffer's `set_frames`, the `InvalidIndex` error variant, the effect
  contract's default `latency_frames`, the chain's `latency_frames`, the
  parameter set's untyped boolean, choice and index readers, the value's
  `from_bits`, the smoothed value's `is_settled`, the table `interpolate`, the
  engine's `ResetChain` and `RefreshDevices` commands and their handlers, the
  `awaiting_recycling` accessors, the `limited_blocks` and `latency_frames`
  counters, and the chain's unread `loaded_preset` field.
- A second pass removed the remaining unreachable API: the core error's
  `InvalidIndex` variant, the engine error's `NoDevice`, `InvalidName` and
  `AlreadyRunning` variants, the preset error's `MissingSection` variant, the
  buffer's `clear`, the chain's `reset` and `is_bypassed`, the engine handle's
  `started`, the preset store's `user_dir`, the autostart helper's `is_enabled`,
  the settings store's `path` and `exists`, `Settings::runs`, the equalizer's
  `gain_values`, the Devices and Preferences views' `page` accessors (and the
  fields behind them), `PreferencesView::developer_mode`, and `State`'s
  `settings_store` accessor. The translation template dropped the five messages
  that only those variants carried (343 messages remain).
- The window activation path matches on the held state directly instead of
  unwrapping it after a separate check, so no `expect` remains in a production
  path.
- The multiband compressor and the multiband gate no longer carry two copies of
  their crossover parameters, band-knob construction and redesign gate; those
  live in `crates/dsp/src/effects/multiband/mod.rs`, and each effect keeps only its
  own parameters and its own per-band maths. The two test modules and the Devices
  test module moved into files of their own (`multiband_compressor/tests.rs`,
  `multiband_gate/tests.rs`, `devices/tests.rs`), which brings every hand-written
  source file back under the 350-line limit.
- The CPU baseline in `docs/architecture.md` was refreshed. The figures there
  predated the three spectral effects, so the full chain read 33x realtime when
  the chain had twenty effects; with all twenty-three it measures about 12x at
  1024 frames and 10x at 128 frames, or 8 to 10 % of one core.

### Documentation

- `docs/feature-matrix.md` moves *Pitch shift* and *Bass loudness* out of the
  unfinished list and into the effect table, with what each transfers from the
  reference and what it reports instead, and its conversion table gained the
  `pitch` and `bass_loudness` rows.
- `docs/architecture.md` describes the destination resolution, the icon resource,
  the split-view layout and how the interface was verified.
- `docs/reference-audit.md` records the completed audit of the reference
  checkout, including what was excluded and why, and how a session manager
  treats a virtual device.
- `docs/feature-matrix.md` documents the reference conversion per effect.
- The stale claim in `docs/reference-audit.md` that auto gain is an RMS
  measurement was corrected: the R128 measurement in `crates/dsp/src/analysis/r128/mod.rs`
  implements the standard, and the audit now says so. The test counts in
  `docs/feature-matrix.md`, `docs/architecture.md` and `README.md` were brought
  in line with the suite at the time, and a merged table row in `scripts/README.md` was
  split again.
- `SECURITY.md` claimed the crate contains no `unsafe` blocks, which stopped
  being true when the gettext FFI was added to `crates/core/src/i18n/mod.rs`. Its
  scope section and design notes now describe that module accurately.
- `SECURITY.md` no longer describes `unsafe` blocks that do not exist in the
  codebase.

### Fixed

- The stereo tools reference conversion no longer lists `dry` and `wet` as
  handled. This project's stereo tools has no dry/wet control, so a reference
  preset that sets them now reports them in the warning list instead of dropping
  them silently. `docs/feature-matrix.md` already recorded them as reported.
- A malformed row in the spectrum analyser table of `docs/feature-matrix.md`
  carried six cells in a four-column table; it is a single row again.
- The harness state list in `docs/architecture.md` and the description of the
  narrow layout now name every state `crates/ui/src/harness/mod.rs` implements and
  the switcher policy it actually sets, and the translation template's message
  count was brought up to date.
- The *Bass loudness* and *Loudness* symbolic icons were drawn from sharp
  rectangles and a sharp polygon, which read as boxy next to the rest of the set.
  Both are now single rounded strokes with `linecap` and `linejoin` set to round.
- The *Crosstalk canceller* icon was redrawn: two speaker cabinets with crossed
  sound paths, because the previous two arcs over two dots read as a bridge and
  was not recognisable at 16 px.

## [0.1.0] - 2026-10-28

The first complete implementation: an output pipeline, thirteen effects, a
generated control surface and a preset library.

### Added

- A virtual PipeWire sink (`pipewire-effects-sink`) built from two `pw_stream`s
  in one node group, with a lock-free single-producer/single-consumer ring
  buffer between them, silence padding on underrun, newest-sample drops on
  overrun and drift trimming on the consumer side.
- Thirteen effects, all implemented in Rust: equalizer, filter, compressor,
  limiter, gate, de-esser, auto gain, bass enhancer, crystalizer, stereo tools,
  gain, delay and reverberation.
- A parameter model in which effects publish metadata (identifier, label, type,
  range, unit, grouping) and keep live values in atomics shared with the audio
  thread. The interface renders any effect from its metadata alone.
- A chain that supports adding, removing, reordering, per-stage enable and
  global bypass, mirrored to the audio thread through a fixed-capacity lock-free
  queue.
- Presets: one built in (Default), JSON on disk, with save, delete, import,
  export, value coercion, name validation, atomic writes and non-fatal
  reporting of unknown content. EasyEffects presets are recognised and
  converted on load, reporting everything that could not be mapped.
- A GTK4/libadwaita interface: navigation split view, parameter editors grouped
  by section, an analytic equalizer response curve drawn with cairo, effect
  picker with search, preset browser, preferences dialog with device selection
  and diagnostics.
- Live input and output meters with peak and RMS, and counters for underruns,
  overruns and drift trims.
- A desktop file and an application icon.

### Design notes

Two properties should survive later changes:

- **The interface is generic.** It renders whatever parameter specifications an
  effect declares, so a new effect needs no interface code.
- **The audio thread never allocates, locks or frees.** Parameter changes use
  atomics, structural edits use a lock-free queue, and removed effects are
  recycled on a non-realtime thread.

The audio path uses two streams rather than one `pw_filter` per effect because
`pipewire-rs` 0.10 does not bind `pw_filter`; the trade-off and the exit path are
recorded in `docs/architecture.md`.

### Verified

- The unit and integration suite, which needs neither a display nor an audio
  device; the current test count is listed in
  [docs/feature-matrix.md](docs/feature-matrix.md).
- Three live tests against a running PipeWire daemon: a tone is fed into the
  virtual sink with `pw-cat` and the level leaving the chain is measured.
  A representative run measured -20.00 dB at the input and -40.00 dB at the
  output for a -20 dB gain stage, and reported 4 096 underrun samples from the
  initial fill with no overruns and no trims.
- A window smoke test (`PIPEWIRE_EFFECTS_SMOKE_TEST=1`) that builds the real
  interface, adds an effect, loads a preset, and reports the engine state,
  device count and negotiated rate.

### Known limitations

- No input (microphone) pipeline and therefore no virtual source.
- Changing the output device rebuilds both streams.
- Drift is corrected by trimming, not resampling.
- The limiter has no lookahead.
- Auto gain is RMS-based rather than EBU R128.
- The default output device is chosen from `priority.session`, not from
  `default.audio.sink` metadata.
- No plugin hosting, FFT convolution, spectrum analyser, neural noise
  suppression, pitch shifting or multiband processing.
- English only; no translations, autostart or packaging.

[Unreleased]: https://github.com/D1ZZY4/PipewireEffects/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/D1ZZY4/PipewireEffects/releases/tag/v0.1.0