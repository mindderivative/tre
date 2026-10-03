"""0.5.4 (#102): `window.set(dpi_scaling=...)` -- layout in logical pixels.

The renderer's half (a scaled frame matches one authored at 2x) is tested in
`engine-render/tests/hidpi.rs` and the conversions in `engine-py`. Here, the
Python surface, and live checks in a real window at a forced scale factor.
"""

import os
import subprocess
import sys
import textwrap

import pytest

from tre import Window


def test_it_is_off_by_default():
    assert Window().get("dpi_scaling") is False


def test_it_can_be_set_and_read_back():
    window = Window()
    window.set(dpi_scaling=True)
    assert window.get("dpi_scaling") is True
    window.set(dpi_scaling=False)
    assert window.get("dpi_scaling") is False


@pytest.mark.parametrize("value", [1, "yes", None, 1.0])
def test_anything_but_a_bool_is_refused_and_changes_nothing(value):
    window = Window()
    window.set(dpi_scaling=True)
    with pytest.raises(ValueError, match="`dpi_scaling` must be a bool"):
        window.set(dpi_scaling=value)
    assert window.get("dpi_scaling") is True


def test_a_window_that_is_not_open_is_at_scale_1():
    window = Window(width=320, height=200)
    window.set(dpi_scaling=True)
    assert (window.get("width"), window.get("height")) == (320, 200)


def test_the_property_is_in_the_unknown_property_errors():
    with pytest.raises(ValueError, match="dpi_scaling"):
        Window().set(colour=1)
    with pytest.raises(ValueError, match="dpi_scaling"):
        Window().get("colour")


LIVE_SCRIPT = """
import sys, threading, time
from tre import App, Window

window = Window(width=320, height=200)
window.set(dpi_scaling=(sys.argv[1] == "on"))
box = window.create("box", width=100, height=60, fill=(255, 0, 0, 255))
window.root.add_child(box)
clicks = []
box.on("pointer_down", lambda e: clicks.append((e.x, e.y)))
app = App()
app.add_window(window)
handle = app.thread_handle()

def report():
    time.sleep(1.0)
    handle.call_soon(lambda: print(
        window.get("scale_factor"), window.get("width"), window.get("height"),
        box.get("layout_width"), flush=True))
    time.sleep(0.5)
    handle.call_soon(window.close)

threading.Thread(target=report, daemon=True).start()
app.run()
"""


def run_live(mode, scale):
    env = dict(os.environ)
    env.pop("WAYLAND_DISPLAY", None)
    env["WINIT_X11_SCALE_FACTOR"] = scale
    result = subprocess.run(
        [sys.executable, "-c", LIVE_SCRIPT, mode],
        env=env, capture_output=True, text=True, timeout=60,
    )
    assert result.returncode == 0, result.stderr
    return [float(x) for x in result.stdout.split()[-4:]]


@pytest.mark.skipif(not os.environ.get("DISPLAY"), reason="needs an X display")
@pytest.mark.parametrize("scale", ["2", "1.5"])
def test_a_scaled_window_lays_out_in_logical_pixels(scale):
    factor, width, height, box = run_live("on", scale)
    assert factor == float(scale)
    assert (width, height) == (320, 200)
    assert box == 100


@pytest.mark.skipif(not os.environ.get("DISPLAY"), reason="needs an X display")
def test_with_it_off_the_window_is_physical_as_before():
    factor, width, height, box = run_live("off", "2")
    assert factor == 2.0
    assert (width, height) == (640, 400)
    assert box == 100
