#!/usr/bin/env bash
set -u

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

out_dir=${OUT_DIR:-"$repo_root/target/ui-screens"}
states=${STATES:-"preset-menu menu menu-developer dev-live devices preferences about"}
scheme=${SCHEME:-prefer-light}
size=${SIZE:-1280x900}
app=${APP:-"$repo_root/target/release/pipewire-effects"}
settle_seconds=${SETTLE_SECONDS:-5}
screen=${SCREEN:-:91}
tone=${TONE:-0}
tone_hz=${TONE_HZ:-440}

usage() {
    cat <<EOF
Capture whole screens of PipeWire Effects, including popovers and dialogs that
live on their own surface and are therefore absent from the window render tree.

Usage: scripts/capture-screen.sh [options]

Environment:
  OUT_DIR          directory for the PNGs        (default target/ui-screens)
  STATES           space separated state names   (default: preset-menu menu menu-developer dev-live devices preferences about)
  SCHEME           prefer-light or prefer-dark   (default prefer-light)
  SIZE             virtual screen geometry       (default 1280x900)
  APP              path to the built binary      (default target/release/pipewire-effects)
  SCREEN           X display of the private Xvfb (default :91)
  SETTLE_SECONDS   wait after the application appears (default 5)
  TONE             play a tone into the application's sink while capturing
                   (default 0, needs ffmpeg and pw-cat)
  TONE_HZ          the tone's frequency          (default 440)

The script needs Xvfb, dbus-run-session and ImageMagick's import. It runs an
isolated X server, a private D-Bus session and a private config directory, and
points only PIPEWIRE_RUNTIME_DIR at the running daemon, so the real session is
not changed. Exit status is non-zero when a state produced no screenshot.

TONE=1 sends a sine wave into the application's own virtual sink for the length
of the capture, which is what makes the meters and the spectrum show real
levels. That tone is played through the destination device, so it is audible:
set the machine's volume first, and leave the switch off when a silent capture
is wanted. It never touches the default sink or the session's stored volumes.
EOF
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    usage
    exit 0
fi

if [ ! -x "$app" ]; then
    echo "capture-screen: $app is not executable; run 'cargo build --release' first" >&2
    exit 2
fi

for tool in Xvfb dbus-run-session import; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "capture-screen: $tool was not found on PATH" >&2
        exit 2
    fi
done

work=$(mktemp -d "${TMPDIR:-/tmp}/pipewire-effects-screen.XXXXXX")
cleanup() {
    rm -rf "$work"
}
trap cleanup EXIT

mkdir -p "$out_dir" "$work/config" "$work/data" "$work/cache"
real_runtime=${PIPEWIRE_RUNTIME_DIR:-${XDG_RUNTIME_DIR:-/run/user/$(id -u)}}

export CAPTURE_SCREEN_WORK="$work"
export CAPTURE_SCREEN_OUT="$out_dir"
export CAPTURE_SCREEN_STATES="$states"
export CAPTURE_SCREEN_SCHEME="$scheme"
export CAPTURE_SCREEN_SIZE="$size"
export CAPTURE_SCREEN_APP="$app"
export CAPTURE_SCREEN_DISPLAY="$screen"
export CAPTURE_SCREEN_SETTLE="$settle_seconds"
export CAPTURE_SCREEN_RUNTIME="$real_runtime"
export CAPTURE_SCREEN_TONE="$tone"
export CAPTURE_SCREEN_TONE_HZ="$tone_hz"

dbus-run-session -- bash -c '
set -u
export XDG_CONFIG_HOME="$CAPTURE_SCREEN_WORK/config"
export XDG_DATA_HOME="$CAPTURE_SCREEN_WORK/data"
export XDG_CACHE_HOME="$CAPTURE_SCREEN_WORK/cache"
export PIPEWIRE_RUNTIME_DIR="$CAPTURE_SCREEN_RUNTIME"
export GSETTINGS_BACKEND=memory
export ADW_DISABLE_PORTAL=1
export GTK_A11Y=none
export NO_AT_BRIDGE=1
export ADW_DEBUG_COLOR_SCHEME="$CAPTURE_SCREEN_SCHEME"
export GDK_BACKEND=x11

Xvfb "$CAPTURE_SCREEN_DISPLAY" -screen 0 "${CAPTURE_SCREEN_SIZE}x24" \
    >"$CAPTURE_SCREEN_WORK/xvfb.log" 2>&1 &
xvfb_pid=$!
export DISPLAY="$CAPTURE_SCREEN_DISPLAY"

for _ in $(seq 1 30); do
    if [ -e "/tmp/.X11-unix/X${CAPTURE_SCREEN_DISPLAY#:}" ]; then
        break
    fi
    sleep 0.5
done

tone_pid=""
stop_tone() {
    if [ -n "$tone_pid" ]; then
        pkill -P "$tone_pid" 2>/dev/null
        kill "$tone_pid" 2>/dev/null
        tone_pid=""
    fi
}

start_tone() {
    if [ "$CAPTURE_SCREEN_TONE" != "1" ]; then
        return
    fi
    for tool in ffmpeg pw-cat; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            echo "capture-screen: TONE=1 needs $tool on PATH" >&2
            return
        fi
    done
    while :; do
        ffmpeg -loglevel error -f lavfi \
            -i "sine=frequency=$CAPTURE_SCREEN_TONE_HZ:sample_rate=48000" \
            -af volume=-20dB -ac 2 -t 30 -f wav - 2>/dev/null \
            | pw-cat --playback --target pipewire-effects-sink - >/dev/null 2>&1
        if ! kill -0 "$app_pid" 2>/dev/null; then
            break
        fi
        sleep 0.5
    done
}

failures=0
for state in $CAPTURE_SCREEN_STATES; do
    safe=$(printf "%s" "$state" | tr ":/" "__")
    png="$CAPTURE_SCREEN_OUT/$safe-$CAPTURE_SCREEN_SCHEME.png"
    log="$CAPTURE_SCREEN_WORK/app-$safe.log"
    rm -f "$png"

    PIPEWIRE_EFFECTS_UI_STATE="$state" "$CAPTURE_SCREEN_APP" >"$log" 2>&1 &
    app_pid=$!

    ready=no
    for _ in $(seq 1 60); do
        owner=$(gdbus call --session \
            --dest org.freedesktop.DBus \
            --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.NameHasOwner \
            io.github.D1ZZY4.PipewireEffects 2>/dev/null)
        case "$owner" in
            *true*) ready=yes; break ;;
        esac
        sleep 0.5
    done
    start_tone &
    tone_pid=$!
    sleep "$CAPTURE_SCREEN_SETTLE"

    if [ "$ready" = "yes" ] && import -window root "$png" 2>>"$log"; then
        printf "  %-14s %s\n" "$state" "$png"
    else
        printf "  %-14s FAILED (see %s)\n" "$state" "$log"
        failures=$((failures + 1))
    fi

    stop_tone
    kill "$app_pid" 2>/dev/null
    wait "$app_pid" 2>/dev/null
done

kill "$xvfb_pid" 2>/dev/null
wait "$xvfb_pid" 2>/dev/null

if [ "$failures" -gt 0 ]; then
    echo "capture-screen: $failures state(s) produced no screenshot" >&2
    exit 4
fi
echo "capture-screen: wrote $(ls -1 "$CAPTURE_SCREEN_OUT" | wc -l) file(s) to $CAPTURE_SCREEN_OUT"
' 2>&1
status=$?
exit "$status"