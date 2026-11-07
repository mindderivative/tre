"""An animation a timer starts must be drawn, not left for the next event to find.

On a headless sway (`tools/sway_live.sh -- python -m pytest tests/test_timer_animation_live.py`).
A frame ticks the animations, then runs the timers that came due; an animation a timer started
was not seen by that tick, and the window, told nothing was animating, drew one frame and went
idle, so the animation sat still until something else woke it. Counts frames with
`Window.frame_stats`, because CPU use cannot tell.
"""

import os
import shutil
import subprocess
import sys
import textwrap

import pytest

CHILD = textwrap.dedent(
    """
    from tess_re import App, Window

    app = App()
    window = Window(width=120, height=80, title="timer-animation")
    box = window.create("box", width=50, height=50, fill=(255, 0, 0, 255))
    window.root.add_child(box)
    app.add_window(window)
    handle = app.thread_handle()
    seen = {}

    def frames(label):
        seen[label] = window.frame_stats(reset=True)["frames"]

    def start():
        frames("startup")
        frames("idle")
        box.animate("rotation_deg", 90.0, 400, easing="linear")

    def finish():
        frames("animating")
        print(seen, flush=True)
        handle.stop()

    window.after(1500, start)
    window.after(2300, finish)
    app.run()
    """
)


def test_an_animation_started_by_a_timer_draws_frames():
    if not (shutil.which("swaymsg") and os.environ.get("SWAYSOCK")):
        pytest.skip("needs a headless sway (SWAYSOCK; tools/sway_live.sh)")
    done = subprocess.run(
        [sys.executable, "-c", CHILD], capture_output=True, text=True, timeout=60, check=False
    )
    assert done.returncode == 0, done.stderr
    seen = eval(done.stdout.strip().splitlines()[-1])  # noqa: S307 - our own child's print
    assert seen["idle"] == 0, "nothing draws before the animation"
    assert seen["animating"] >= 10, f"a 400 ms animation drew {seen['animating']} frames"
