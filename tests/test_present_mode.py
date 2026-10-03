"""0.5.4 (#101): `window.set(present_mode=...)` -- how the swapchain paces frames.

The choice itself (which `wgpu` mode each name maps to, and the fallbacks) is
unit-tested in `engine-render`. Here, the Python surface, and one live check
that a continuously animating window is paced.
"""

import os
import subprocess
import sys
import textwrap

import pytest

from tre import Window


def test_vsync_is_the_default():
    assert Window().get("present_mode") == "vsync"


@pytest.mark.parametrize("mode", ["vsync", "low_latency"])
def test_the_modes_can_be_set_and_read_back(mode):
    window = Window()
    window.set(present_mode="low_latency")
    window.set(present_mode=mode)
    assert window.get("present_mode") == mode


@pytest.mark.parametrize("value", ["mailbox", "", "VSYNC", 1, None, True])
def test_anything_else_is_refused_and_changes_nothing(value):
    window = Window()
    window.set(present_mode="low_latency")
    with pytest.raises(ValueError, match='`present_mode` must be "vsync" or "low_latency"'):
        window.set(present_mode=value)
    assert window.get("present_mode") == "low_latency"


def test_the_property_is_in_the_unknown_property_errors():
    with pytest.raises(ValueError, match="present_mode"):
        Window().set(colour=1)
    with pytest.raises(ValueError, match="present_mode"):
        Window().get("colour")


ANIMATE_SCRIPT = """
import os, sys, threading, time
from tre import App, Window

window = Window(width=320, height=200)
box = window.create("box", width=60, height=40, fill=(255, 0, 0, 255))
window.root.add_child(box)
box.animate("opacity", 0.2, 600000)          # a long animation: frames until we stop
if len(sys.argv) > 1:
    window.set(present_mode=sys.argv[1])
app = App()
app.add_window(window)
handle = app.thread_handle()
started = []
handle.call_soon(lambda: started.append(True))
threading.Thread(target=lambda: (time.sleep(5.0), handle.call_soon(window.close)), daemon=True).start()
app.run()
if not started:
    print("NO_FRAMES")
else:
    print("CPU", round(time.process_time(), 2))
"""


def animate_cpu_seconds(mode: str | None) -> float:
    script = textwrap.dedent(ANIMATE_SCRIPT)
    args = [sys.executable, "-c", script] + ([mode] if mode else [])
    done = subprocess.run(args, capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    if "NO_FRAMES" in done.stdout:
        pytest.skip("no display reachable")
    return float(done.stdout.strip().splitlines()[-1].split()[1])


@pytest.mark.skipif(
    not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")),
    reason="no display reachable",
)
def test_an_animating_window_is_paced_by_default():
    """Five seconds of animation: unpaced (`Mailbox`) it used a whole core
    (about 5 s of CPU); paced, a small fraction. The bound is loose, since
    CI runners differ, and what it separates is a core from a few percent."""
    paced = animate_cpu_seconds(None)
    assert paced < 2.5, f"an animating window used {paced:.2f}s of CPU in 5s: not paced"
