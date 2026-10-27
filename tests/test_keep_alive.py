"""0.5.9 (#238): `App.run(keep_alive=True)` and `LoopHandle.stop()`.

A desktop shell has to stay up with no window (every monitor set to show nothing, the last
output unplugged), keep hearing about outputs, and end cleanly on quit. Without `keep_alive`
nothing changes: `run()` raises with no window and ends with the last one.

The cases that need an event loop run in a child process (a second real `App.run()` in the test
process breaks the others) and skip without a display; the hot-plug ones need a headless sway
(`tools/sway_live.sh`) because they create and unplug its outputs.
"""

import json
import os
import select
import shutil
import subprocess
import sys
import textwrap
import time

import pytest

from tre import App, Window

needs_display = pytest.mark.skipif(
    not (os.environ.get("WAYLAND_DISPLAY") or os.environ.get("DISPLAY")),
    reason="needs a display for the event loop",
)

PRELUDE = """
import json, os, sys, threading, time
from tre import App, LayerWindow, Window
import tre
T0 = time.monotonic()
def say(**kw):
    print(json.dumps({"t": round(time.monotonic() - T0, 2), **kw}), flush=True)
"""


def child(code, timeout=40, **env):
    """Runs `code` (after PRELUDE) in a fresh interpreter; returns its JSON lines and the exit status."""
    done = subprocess.run(
        [sys.executable, "-c", PRELUDE + textwrap.dedent(code)],
        capture_output=True, text=True, timeout=timeout, env=dict(os.environ, **env),
    )
    lines = [json.loads(line) for line in done.stdout.splitlines() if line.startswith("{")]
    return lines, done


def find(lines, key):
    return [line[key] for line in lines if key in line]


# --- no loop needed --------------------------------------------------------------


def test_run_still_raises_with_no_window_unless_asked_to_keep_alive():
    with pytest.raises(RuntimeError, match="no windows"):
        App().run()
    with pytest.raises(RuntimeError, match="no windows"):
        App().run(keep_alive=False)


def test_stop_outside_a_run_is_false_and_remembers_nothing():
    handle = App().thread_handle()
    assert handle.stop() is False
    assert handle.stop() is False


# --- zero windows ----------------------------------------------------------------


@needs_display
def test_a_keep_alive_loop_with_no_window_runs_callbacks_until_stopped():
    lines, done = child("""
        app = App()
        h = app.thread_handle()
        say(stop_before=h.stop())
        def script():
            time.sleep(1.2)
            ran = threading.Event()
            h.call_soon(lambda: (say(call_soon="ran with no window"), ran.set()))
            ran.wait(3)
            say(stop=h.stop())
        threading.Thread(target=script, daemon=True).start()
        app.run(keep_alive=True)
        say(returned=True, stop_after=h.stop())
    """)
    assert done.returncode == 0, done.stderr
    assert find(lines, "stop_before") == [False], "a stop before the run is not remembered"
    assert find(lines, "call_soon") == ["ran with no window"]
    assert find(lines, "stop") == [True]
    assert find(lines, "stop_after") == [False]
    returned = next(line for line in lines if line.get("returned"))
    assert returned["t"] >= 1.2, "the loop did not end on its own"


@needs_display
def test_a_stop_asked_before_the_run_does_not_end_the_next_run():
    lines, done = child("""
        app = App()
        h = app.thread_handle()
        h.stop()
        threading.Thread(target=lambda: (time.sleep(1.5), h.stop()), daemon=True).start()
        app.run(keep_alive=True)
        say(returned=True)
    """)
    assert done.returncode == 0, done.stderr
    assert next(line for line in lines if line.get("returned"))["t"] >= 1.4


@needs_display
def test_an_idle_loop_with_no_window_uses_no_cpu_and_wakes_no_engine_thread():
    """The shell's requirement: no frames, no timers. 5 s with no window: the process uses about
    nothing, and none of tre's own threads (named `tre-*`) wakes on a timer."""
    lines, done = child("""
        import glob
        def tre_switches():
            total = 0
            for d in glob.glob("/proc/self/task/*"):
                try:
                    if not open(d + "/comm").read().startswith("tre-"):
                        continue
                    st = open(d + "/status").read()
                    total += sum(int(st.split(key + ":")[1].split()[0])
                                 for key in ("voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"))
                except OSError:
                    pass
            return total
        def cpu():
            p = open("/proc/self/stat").read().rsplit(")", 1)[1].split()
            return (int(p[11]) + int(p[12])) / os.sysconf("SC_CLK_TCK")
        app = App()
        h = app.thread_handle()
        def script():
            time.sleep(1.5)
            c0, s0 = cpu(), tre_switches()
            time.sleep(5.0)
            say(cpu_seconds=round(cpu() - c0, 3), engine_thread_switches=tre_switches() - s0)
            h.stop()
        threading.Thread(target=script, daemon=True).start()
        app.run(keep_alive=True)
    """, timeout=60)
    assert done.returncode == 0, done.stderr
    assert find(lines, "cpu_seconds")[0] < 0.15, lines
    assert find(lines, "engine_thread_switches")[0] <= 4, lines


@needs_display
def test_sigterm_ends_an_idle_loop_through_a_wakeup_fd_and_a_watcher_thread():
    """The recipe for quitting a window-less shell on a signal: a Python handler does not run
    on an idle loop (the main thread is not executing Python), but `signal.set_wakeup_fd` is
    written by the C handler at once, and a watcher thread calls `stop()`."""
    lines, done = child("""
        import signal, socket
        app = App()
        h = app.thread_handle()
        reader, writer = socket.socketpair()
        writer.setblocking(False)
        signal.set_wakeup_fd(writer.fileno())
        signal.signal(signal.SIGTERM, lambda s, f: None)   # a handler must exist for the fd to be written
        def watcher():
            data = reader.recv(8)
            say(signal=signal.Signals(data[0]).name, stop=h.stop())
        threading.Thread(target=watcher, daemon=True).start()
        threading.Thread(target=lambda: (time.sleep(1.5), os.kill(os.getpid(), signal.SIGTERM)), daemon=True).start()
        app.run(keep_alive=True)
        say(returned=True)
    """)
    assert done.returncode == 0, done.stderr
    assert find(lines, "signal") == ["SIGTERM"] and find(lines, "stop") == [True]
    returned = next(line for line in lines if line.get("returned"))
    assert 1.4 <= returned["t"] < 4, "ended soon after the signal, not before it and not at a timeout"


# --- windows ---------------------------------------------------------------------


@needs_display
def test_stop_closes_every_window_without_asking_and_run_returns_normally():
    lines, done = child("""
        app = App()
        h = app.thread_handle()
        asked = []
        windows = []
        for n in range(3):
            w = Window(width=120, height=80)
            w.on("close_requested", lambda e, n=n: (asked.append(n), e.cancel()))
            w.on("closed", lambda e, n=n: say(closed=n, reason=e.reason))
            app.add_window(w)
            windows.append(w)
        if tre.supports("layer_shell"):
            bar = LayerWindow("stop-test", anchor=("top", "left", "right"), size=(0, 24), exclusive_zone=0)
            bar.on("close_requested", lambda e: (asked.append("bar"), e.cancel()))
            bar.on("closed", lambda e: say(closed="bar", reason=e.reason))
            app.add_window(bar)
        threading.Thread(target=lambda: (time.sleep(1.5), say(stop=h.stop())), daemon=True).start()
        app.run()
        say(returned=True, asked=asked)
    """)
    assert done.returncode == 0, done.stderr
    assert find(lines, "stop") == [True]
    closed = {line["closed"]: line["reason"] for line in lines if "closed" in line}
    assert {0, 1, 2} <= set(closed), closed
    assert set(closed.values()) == {"stopped"}, closed
    assert next(line for line in lines if line.get("returned"))["asked"] == [], "no close_requested was asked"


@needs_display
@pytest.mark.parametrize("keep_alive, ends_with_the_window", [(False, True), (True, False)])
def test_closing_the_last_window_ends_the_run_only_without_keep_alive(keep_alive, ends_with_the_window):
    lines, done = child(f"""
        app = App()
        h = app.thread_handle()
        w = Window(width=120, height=80)
        w.on("closed", lambda e: say(closed=True))
        app.add_window(w)
        threading.Thread(target=lambda: (time.sleep(1.2), h.call_soon(w.close)), daemon=True).start()
        threading.Thread(target=lambda: (time.sleep(3.0), h.stop()), daemon=True).start()
        app.run(keep_alive={keep_alive})
        say(returned=True)
    """)
    assert done.returncode == 0, done.stderr
    returned = next(line for line in lines if line.get("returned"))["t"]
    assert find(lines, "closed") == [True]
    if ends_with_the_window:
        assert returned < 2.6, "the run ended with its last window"
    else:
        assert returned >= 2.9, "the run waited for stop()"


# --- outputs, on a headless sway ------------------------------------------------


def _sway():
    if not (shutil.which("swaymsg") and os.environ.get("SWAYSOCK")):
        pytest.skip("needs a headless sway (SWAYSOCK; tools/sway_live.sh)")
    out = subprocess.run(["swaymsg", "-t", "get_outputs"], capture_output=True, text=True)
    if out.returncode != 0:
        pytest.skip("SWAYSOCK does not reach a running sway")
    return {o["name"] for o in json.loads(out.stdout)}


def test_with_no_window_output_events_keep_arriving_and_a_bar_can_be_added_from_one():
    names = _sway()
    lines, done = child("""
        import subprocess
        def sway(*a): subprocess.run(["swaymsg", *a], capture_output=True)
        app = App()
        say(outputs=sorted(o.name for o in app.outputs()))
        def added(o):
            say(event="output_added", output=o.name)
            bar = LayerWindow("hot-" + o.name, anchor=("top", "left", "right"), size=(0, 20), exclusive_zone=0, output=o.name)
            bar.root.set(fill=(1, 2, 3, 255))
            bar.on("configure", lambda e: say(configure=o.name))
            bar.on("closed", lambda e: say(closed=o.name, reason=e.reason))
            app.add_window(bar)
        app.on("output_added", added)
        app.on("output_removed", lambda o: say(event="output_removed", output=o.name))
        h = app.thread_handle()
        def script():
            time.sleep(1.0)
            sway("create_output"); time.sleep(1.5)
            for name in sorted(o.name for o in app.outputs()):
                sway("output", name, "unplug"); time.sleep(1.0)
            say(step="no output left")
            sway("create_output"); time.sleep(2.0)
            say(stop=h.stop())
        threading.Thread(target=script, daemon=True).start()
        app.run(keep_alive=True)
        say(returned=True)
    """, timeout=60)
    assert done.returncode == 0, done.stderr
    assert set(find(lines, "outputs")[0]) == names, "the child is on the sway under test"
    added = [line["output"] for line in lines if line.get("event") == "output_added"]
    removed = [line["output"] for line in lines if line.get("event") == "output_removed"]
    assert len(added) == 2, lines  # one before the unplugging, one after the last output went
    assert len(removed) >= 2, lines
    assert set(find(lines, "configure")) == set(added), "a bar opened on each output it was added for"
    assert find(lines, "stop") == [True]
    assert any(line.get("returned") for line in lines), "the run survived the last output going away"
    reasons = {line["reason"] for line in lines if "closed" in line}
    assert reasons <= {"output_removed", "stopped"}, reasons
