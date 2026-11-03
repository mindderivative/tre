"""0.5.10 (#246): a window the compositor is not showing must not stall the others.

On a headless sway (`tools/sway_live.sh -- python -m pytest tests/test_hidden_window_live.py`).
Before the fix a window moved off screen left a vsync present blocked, with the
Python GIL held, so the whole app froze: every other window and every Python thread.
"""

import os
import shutil
import subprocess
import sys
import textwrap
import time

import pytest

CHILD = textwrap.dedent(
    """
    import sys, threading, time
    from tess_re import App, Window

    app = App()
    for title in ("hide-me", "stay"):
        window = Window(width=100, height=100, title=title)
        node = window.create("box", width=40, height=40, position="absolute", x=10, y=10,
                             fill=(200, 50, 50, 255))
        window.root.add_child(node)
        node.animate("rotation_deg", 360000.0, 3_600_000, easing="linear")
        app.add_window(window)
    handle = app.thread_handle()

    def beat():
        n = 0
        while True:
            time.sleep(0.1)
            n += 1
            print(n, flush=True)
            if n > 600:
                handle.stop()
                return

    threading.Thread(target=beat, daemon=True).start()
    app.run()
    """
)


def _sway():
    if not (shutil.which("swaymsg") and os.environ.get("SWAYSOCK")):
        pytest.skip("needs a headless sway (SWAYSOCK; tools/sway_live.sh)")
    if (
        subprocess.run(
            ["swaymsg", "-t", "get_outputs"], check=False, capture_output=True
        ).returncode
        != 0
    ):
        pytest.skip("SWAYSOCK does not reach a running sway")


def _grab():
    """The top 200 x 30 pixels of the screen, where the bar is, as PNG bytes."""
    return subprocess.run(
        ["grim", "-g", "0,0 200x30", "-"], capture_output=True, check=False
    ).stdout


def _beats(proc, seconds):
    """How many heartbeat lines the child's Python thread printed in `seconds`."""
    os.set_blocking(proc.stdout.fileno(), False)
    count, end = 0, time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            line = proc.stdout.readline()
        except BlockingIOError:
            line = None
        if line:
            count += 1
        else:
            time.sleep(0.02)
    return count


def test_a_hidden_window_does_not_freeze_the_other_window_or_python_threads():
    _sway()
    proc = subprocess.Popen(
        [sys.executable, "-c", CHILD],
        stdout=subprocess.PIPE,
        text=True,
        stderr=subprocess.DEVNULL,
    )
    try:
        assert _beats(proc, 3.0) > 15, (
            "the child must be running with both windows open"
        )
        subprocess.run(
            ["swaymsg", '[title="hide-me"] move scratchpad'], capture_output=True
        )
        time.sleep(1.0)
        # 2 s at one beat per 0.1 s is 20; a frozen app prints none.
        assert _beats(proc, 2.0) >= 12
        subprocess.run(
            ["swaymsg", '[title="hide-me"] scratchpad show'], capture_output=True
        )
        assert _beats(proc, 2.0) >= 12, "and it goes on after the window comes back"
    finally:
        proc.kill()
        proc.wait()


LAYER_CHILD = textwrap.dedent(
    """
    import threading, time
    from tess_re import App, LayerWindow, Window

    app = App()
    bar = LayerWindow("probe-bar", anchor=("top", "left", "right"), size=(0, 30),
                      exclusive_zone=0, layer="top")
    spinner = bar.create("box", width=20, height=20, position="absolute", x=5, y=5,
                         fill=(50, 200, 50, 255))
    bar.root.add_child(spinner)
    spinner.animate("rotation_deg", 360000.0, 3_600_000, easing="linear")
    app.add_window(bar)
    window = Window(width=100, height=100, title="cover")
    window.root.add_child(window.create("box", width=40, height=40, fill=(200, 50, 50, 255)))
    app.add_window(window)
    handle = app.thread_handle()

    def beat():
        for n in range(1, 600):
            time.sleep(0.1)
            print(n, flush=True)
        handle.stop()

    threading.Thread(target=beat, daemon=True).start()
    app.run()
    """
)


def test_a_layer_surface_under_a_fullscreen_window_does_not_freeze_the_app():
    _sway()
    proc = subprocess.Popen(
        [sys.executable, "-c", LAYER_CHILD],
        stdout=subprocess.PIPE,
        text=True,
        stderr=subprocess.DEVNULL,
    )
    try:
        assert _beats(proc, 3.0) > 15, (
            "the child must be running with both windows open"
        )
        subprocess.run(
            ["swaymsg", '[title="cover"] fullscreen enable'],
            capture_output=True,
            check=False,
        )
        time.sleep(1.0)
        assert _beats(proc, 2.0) >= 12
        subprocess.run(
            ["swaymsg", '[title="cover"] fullscreen disable'],
            capture_output=True,
            check=False,
        )
        time.sleep(1.0)
        assert _beats(proc, 2.0) >= 12
        if shutil.which("grim"):
            first = _grab()
            time.sleep(0.4)
            assert first != _grab(), "the bar draws again once it is uncovered"
    finally:
        proc.kill()
        proc.wait()
