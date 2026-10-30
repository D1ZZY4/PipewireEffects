#!/usr/bin/env bash
set -u

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

out_dir=${OUT_DIR:-"$repo_root/target/ui-captures"}
states=${STATES:-"chain editor equalizer presets devices preferences developer picker picker-tone about banner bypass narrow mobile mobile-drawer sidebar-hidden"}
scheme=${SCHEME:-prefer-light}
size=${SIZE:-1280x900}
app=${APP:-"$repo_root/target/release/pipewire-effects"}
display_name=${WAYLAND_DISPLAY_NAME:-wayland-31}
settle_seconds=${SETTLE_SECONDS:-3}
capture_timeout=${CAPTURE_TIMEOUT:-40}
state_attempts=${CAPTURE_ATTEMPTS:-3}
keep_workdir=${KEEP_WORKDIR:-0}

usage() {
    cat <<EOF
Capture the interface of PipeWire Effects on an isolated headless GNOME session.

Usage: scripts/capture-ui.sh [options]

Environment:
  OUT_DIR          directory for the PNGs        (default target/ui-captures)
  STATES           space separated state names   (default: the standard set)
  SCHEME           prefer-light or prefer-dark   (default prefer-light)
  SIZE             virtual monitor geometry      (default 1280x900)
  APP              path to the built binary      (default target/release/pipewire-effects)
  WAYLAND_DISPLAY_NAME  private Wayland socket   (default wayland-31)
  SETTLE_SECONDS   wait after the shell is ready (default 3)
  CAPTURE_TIMEOUT  seconds to wait for one PNG   (default 40)
  CAPTURE_ATTEMPTS launcher retries per state    (default 3)
  KEEP_WORKDIR     set to 1 to keep the logs and the private config

States: chain editor equalizer presets devices preferences developer picker
        picker-tone about banner bypass narrow mobile mobile-drawer
        sidebar-hidden input effect:<kind>

The script needs gnome-shell, dbus-run-session and a running PipeWire daemon.
It never changes the real session: the Wayland socket, the config directory and
the settings live in a private temporary tree, and only PIPEWIRE_RUNTIME_DIR
points at the running daemon so the engine can connect.

Exit status is non-zero when a state produced no screenshot.
EOF
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    usage
    exit 0
fi

if [ ! -x "$app" ]; then
    echo "capture-ui: $app is not executable; run 'cargo build --release' first" >&2
    exit 2
fi

for tool in gnome-shell dbus-run-session; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "capture-ui: $tool was not found on PATH" >&2
        exit 2
    fi
done

work=$(mktemp -d "${TMPDIR:-/tmp}/pipewire-effects-capture.XXXXXX")
cleanup() {
    if [ "$keep_workdir" = "1" ]; then
        echo "capture-ui: work directory kept at $work"
    else
        rm -rf "$work"
    fi
}
trap cleanup EXIT

mkdir -p "$out_dir" "$work/run" "$work/config" "$work/data" "$work/logs"
chmod 700 "$work/run"

real_runtime=${PIPEWIRE_RUNTIME_DIR:-${XDG_RUNTIME_DIR:-/run/user/$(id -u)}}

export CAPTURE_WORK="$work"
export CAPTURE_OUT="$out_dir"
export CAPTURE_STATES="$states"
export CAPTURE_SCHEME="$scheme"
export CAPTURE_SIZE="$size"
export CAPTURE_APP="$app"
export CAPTURE_DISPLAY="$display_name"
export CAPTURE_SETTLE="$settle_seconds"
export CAPTURE_TIMEOUT="$capture_timeout"
export CAPTURE_ATTEMPTS="$state_attempts"
export CAPTURE_REAL_RUNTIME="$real_runtime"

dbus-run-session -- bash -c '
set -u
export XDG_RUNTIME_DIR="$CAPTURE_WORK/run"
export XDG_CONFIG_HOME="$CAPTURE_WORK/config"
export XDG_DATA_HOME="$CAPTURE_WORK/data"
export XDG_CACHE_HOME="$CAPTURE_WORK/cache"
export PIPEWIRE_RUNTIME_DIR="$CAPTURE_REAL_RUNTIME"
export GSETTINGS_BACKEND=memory
export ADW_DISABLE_PORTAL=1
export GTK_A11Y=none
export NO_AT_BRIDGE=1
export ADW_DEBUG_COLOR_SCHEME="$CAPTURE_SCHEME"

shell_log="$CAPTURE_WORK/logs/gnome-shell.log"
gnome-shell --headless --wayland --wayland-display="$CAPTURE_DISPLAY" \
    --virtual-monitor "$CAPTURE_SIZE" >"$shell_log" 2>&1 &
shell_pid=$!
stop_shell() {
    pkill -P "$shell_pid" 2>/dev/null
    kill "$shell_pid" 2>/dev/null
}
trap stop_shell EXIT

for _ in $(seq 1 90); do
    if [ -S "$XDG_RUNTIME_DIR/$CAPTURE_DISPLAY" ]; then
        break
    fi
    sleep 1
done
if [ ! -S "$XDG_RUNTIME_DIR/$CAPTURE_DISPLAY" ]; then
    echo "capture-ui: the headless shell never created $CAPTURE_DISPLAY" >&2
    exit 3
fi

ready=no
for _ in $(seq 1 60); do
    owner=$(gdbus call --session \
        --dest org.freedesktop.DBus \
        --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.NameHasOwner org.gnome.Shell 2>/dev/null)
    case "$owner" in
        *true*) ready=yes; break ;;
    esac
    sleep 1
done
if [ "$ready" != "yes" ]; then
    echo "capture-ui: org.gnome.Shell did not appear on the private bus" >&2
    exit 3
fi
sleep "$CAPTURE_SETTLE"

failures=0
for state in $CAPTURE_STATES; do
    safe=$(printf "%s" "$state" | tr ":/" "__")
    png="$CAPTURE_OUT/$safe-$CAPTURE_SCHEME.png"
    log="$CAPTURE_WORK/logs/app-$safe.log"
    rm -f "$png"

    attempt=0
    while [ ! -f "$png" ] && [ "$attempt" -lt "$CAPTURE_ATTEMPTS" ]; do
        attempt=$((attempt + 1))
        WAYLAND_DISPLAY="$CAPTURE_DISPLAY" GDK_BACKEND=wayland \
            PIPEWIRE_EFFECTS_UI_STATE="$state" \
            PIPEWIRE_EFFECTS_UI_CAPTURE="$png" \
            "$CAPTURE_APP" >>"$log" 2>&1 &
        app_pid=$!

        waited=0
        while [ ! -f "$png" ] && [ "$waited" -lt "$CAPTURE_TIMEOUT" ]; do
            if ! kill -0 "$app_pid" 2>/dev/null; then
                break
            fi
            sleep 0.5
            waited=$((waited + 1))
        done
        kill "$app_pid" 2>/dev/null
        wait "$app_pid" 2>/dev/null

        if [ ! -f "$png" ] && [ "$attempt" -lt "$CAPTURE_ATTEMPTS" ]; then
            sleep 1
        fi
    done

    if [ -f "$png" ]; then
        line=$(grep -m1 "UI_CAPTURE" "$log" 2>/dev/null)
        printf "  %-14s %s\n" "$state" "${line:-captured}"
    else
        printf "  %-14s FAILED after %s attempt(s) (see %s)\n" "$state" "$attempt" "$log"
        failures=$((failures + 1))
    fi
done

if [ "$failures" -gt 0 ]; then
    echo "capture-ui: $failures state(s) produced no screenshot" >&2
    exit 4
fi
echo "capture-ui: wrote $(ls -1 "$CAPTURE_OUT" | wc -l) file(s) to $CAPTURE_OUT"
'
status=$?
exit "$status"
