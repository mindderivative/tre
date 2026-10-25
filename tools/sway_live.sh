#!/usr/bin/env bash
# Run a command inside a private headless sway, then shut sway down. For the live LayerWindow tests.
#
#     tools/sway_live.sh -- python -m pytest tests/test_layer_window_live.py -q
#     tools/sway_live.sh -- swaymsg -t get_outputs
#     tools/sway_live.sh --renderer pixman -- ...    # compositor on the CPU, no GPU for sway itself
#
# What it does, so it is not done by hand: starts `sway -c <config>` with WLR_BACKENDS=headless
# beside the real session (its own wayland-N socket, so KWin is left alone), waits until sway answers
# on its IPC socket, exports WAYLAND_DISPLAY and SWAYSOCK for the command, runs it, and on the way
# out asks sway to exit, then TERM, then KILL, by the pid it started (never by a name pattern).
# The command's exit status is returned. XDG_RUNTIME_DIR must be set (it is in a login session).
#
# --renderer gles2 (the default) matches the earlier live runs and uses the real GPU for the
# compositor; pixman keeps the compositor off the GPU. The tests' own windows still render with
# wgpu on whatever adapter they pick (see tools/with_adapter.sh).
# Not for KWin-only checks; those need TRE_LIVE_INPUT=1 and a real session.
set -uo pipefail

renderer=gles2
while [ $# -gt 0 ]; do
    case "$1" in
        --renderer) renderer="${2:-}"; shift 2 ;;
        --) shift; break ;;
        -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) break ;;
    esac
done
[ "$#" -gt 0 ] || { echo "sway_live.sh: no command given (put it after --)" >&2; exit 2; }
command -v sway >/dev/null || { echo "sway_live.sh: sway is not installed" >&2; exit 2; }
command -v swaymsg >/dev/null || { echo "sway_live.sh: swaymsg is not installed" >&2; exit 2; }
runtime="${XDG_RUNTIME_DIR:-}"
[ -d "$runtime" ] || { echo "sway_live.sh: XDG_RUNTIME_DIR is not set" >&2; exit 2; }

work=$(mktemp -d "${TMPDIR:-/tmp}/sway-live.XXXXXX")
pid=""
cleanup() {
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        SWAYSOCK="$sock" swaymsg exit >/dev/null 2>&1
        for _ in $(seq 1 50); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
        kill -0 "$pid" 2>/dev/null && { kill -TERM "$pid" 2>/dev/null; sleep 1; }
        kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
    fi
    rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# sway runs this on start-up and tells us which wayland socket it chose.
printf 'exec sh -c %q\n' "echo \"\$WAYLAND_DISPLAY\" > '$work/display'" > "$work/config"

WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER="$renderer" \
    sway -c "$work/config" >"$work/sway.log" 2>&1 &
pid=$!
sock="$runtime/sway-ipc.$(id -u).$pid.sock"

for _ in $(seq 1 100); do
    [ -S "$sock" ] && [ -s "$work/display" ] && SWAYSOCK="$sock" swaymsg -t get_version >/dev/null 2>&1 && break
    kill -0 "$pid" 2>/dev/null || { echo "sway_live.sh: sway exited early:" >&2; tail -20 "$work/sway.log" >&2; exit 1; }
    sleep 0.1
done
if ! [ -s "$work/display" ]; then
    echo "sway_live.sh: sway did not come up in 10 s:" >&2; tail -20 "$work/sway.log" >&2; exit 1
fi

export SWAYSOCK="$sock" WAYLAND_DISPLAY
WAYLAND_DISPLAY=$(cat "$work/display")
echo "sway_live: pid $pid, WAYLAND_DISPLAY=$WAYLAND_DISPLAY, renderer $renderer" >&2
"$@"
status=$?
exit "$status"
