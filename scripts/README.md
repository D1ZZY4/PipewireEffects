# Scripts

Automation that supports development and verification. Nothing here is part of
the shipped application.

## `capture-ui.sh`

Renders the real interface on an isolated headless GNOME session and writes one
PNG per requested screen state. It is the tool behind the interface review in
`docs/architecture.md`.

What it does, in order:

1. Starts `dbus-run-session` with a private `XDG_RUNTIME_DIR`, `XDG_CONFIG_HOME`,
   `XDG_DATA_HOME` and `XDG_CACHE_HOME` under a temporary directory, so the run
   cannot touch your real settings, presets or autostart entry.
2. Starts `gnome-shell --headless --wayland --virtual-monitor` on a private
   Wayland display and waits until the socket exists and `org.gnome.Shell` owns
   its bus name.
3. Launches the release binary once per state with `PIPEWIRE_EFFECTS_UI_STATE`
   and `PIPEWIRE_EFFECTS_UI_CAPTURE` set, waits for the PNG the application
   writes, then stops the process.
4. Stops the shell and removes the temporary tree unless `KEEP_WORKDIR=1`.

`PIPEWIRE_RUNTIME_DIR` is the only variable that points at the running session,
so the engine connects to your PipeWire daemon and creates its own virtual
nodes. The script never changes the default sink or source, never restarts
PipeWire and never writes to your configuration.

Because GNOME refuses `org.gnome.Shell.Screenshot` to other clients, the PNG is
the window's own render tree through `PIPEWIRE_EFFECTS_UI_CAPTURE`, not a
compositor capture of the screen.

### Usage

```sh
cargo build --release
scripts/capture-ui.sh
```

All configuration is through the environment:

| Variable | Default | Meaning |
|---|---|---|
| `OUT_DIR` | `target/ui-captures` | output directory for the PNGs |
| `STATES` | the standard set | space separated state names |
| `SCHEME` | `prefer-light` | `prefer-light` or `prefer-dark` |
| `SIZE` | `1280x900` | virtual monitor geometry |
| `APP` | `target/release/pipewire-effects` | the binary to run |
| `WAYLAND_DISPLAY_NAME` | `wayland-31` | private Wayland socket name |
| `SETTLE_SECONDS` | `3` | pause after the shell reports ready |
| `CAPTURE_TIMEOUT` | `40` | seconds to wait for one PNG |
| `KEEP_WORKDIR` | `0` | keep the logs and private configuration |

The file is named `<state>-<scheme>.png`; `:` in a state name becomes `_`.

Examples:

```sh
# Both appearances, the standard set
scripts/capture-ui.sh
SCHEME=prefer-dark scripts/capture-ui.sh

# One screen, custom output
STATES="equalizer" OUT_DIR=/tmp/eq scripts/capture-ui.sh

# One editor per effect
STATES="$(for k in equalizer filter compressor limiter gate deesser autogain \
  bass_enhancer crystalizer crusher exciter stereo_tools crossfeed crosstalk \
  gain delay reverb expander maximizer midside_equalizer; do \
    printf 'effect:%s ' "$k"; done)" scripts/capture-ui.sh
```

The exit status is non-zero when a state produced no screenshot, so the script
can gate a review.

### States

| State | What it shows |
|---|---|
| `chain` | the Effects page with the built-in preset loaded |
| `editor` | the Effects page with the third effect selected |
| `equalizer` | the equalizer editor with its response curve |
| `presets` | the preset library |
| `devices` | the Devices dialog, opened through the `win.devices` action |
| `preferences` | the Preferences dialog |
| `developer` | Preferences with developer mode on and the diagnostics expanded |
| `picker` | the Add-effect dialog |
| `picker-tone` | the Add-effect dialog scrolled to its end |
| `about` | the About dialog |
| `banner` | an error banner over the Effects page |
| `bypass` | the Effects page with the chain bypassed |
| `narrow` | the collapsed layout at 720x620 |
| `input` | the Input pipeline, with microphone processing switched on |
| `effect:<kind>` | the built-in preset plus the named effect, selected |

`input` starts the real microphone pipeline for the duration of the capture.
Use it knowingly; it records nothing, but it does open the capture device.

The PNGs are evidence for a review, not proof that a screen is correct or that a
control works. Inspect them and exercise the application as well.
