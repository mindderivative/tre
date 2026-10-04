"""0.5.4 (#137): transparent windows (`transparent`, `transparent_active`,
`blur_behind`), and the straight-alpha snapshot a translucent window needs.
"""

import os
import subprocess
import sys
import textwrap

import pytest

from tre import Window


def test_the_defaults_are_opaque_and_unknown_until_open():
    window = Window()
    assert window.get("transparent") is False
    assert window.get("transparent_active") is None
    assert window.get("blur_behind") is False


def test_transparent_and_blur_behind_can_be_set_before_the_window_opens():
    window = Window()
    window.set(transparent=True, blur_behind=True)
    assert (window.get("transparent"), window.get("blur_behind")) == (True, True)
    window.set(transparent=False)
    assert window.get("transparent") is False


@pytest.mark.parametrize("name", ["transparent", "blur_behind"])
@pytest.mark.parametrize("value", [1, "yes", None])
def test_they_take_only_a_bool(name, value):
    with pytest.raises(ValueError, match=f"`{name}` must be a bool"):
        Window().set(**{name: value})


def test_transparent_active_is_read_only():
    with pytest.raises(ValueError, match="read-only"):
        Window().set(transparent_active=True)


def test_a_snapshot_is_straight_alpha_so_translucent_pixels_keep_their_colour():
    window = Window(width=60, height=40)
    window.root.set(fill=(0, 0, 0, 0), padding=0)
    box = window.create("box", width=20, height=20, x=10, y=10, position="absolute",
                        fill=(255, 0, 0, 128))
    window.root.add_child(box)
    rgba, width, _ = window.snapshot()
    px = lambda x, y: tuple(rgba[(y * width + x) * 4 : (y * width + x) * 4 + 4])  # noqa: E731
    assert px(2, 2) == (0, 0, 0, 0), "clear stays clear"
    r, g, b, a = px(20, 20)
    assert (g, b) == (0, 0) and abs(a - 128) <= 1
    assert r >= 254, f"straight alpha keeps the colour full: {r}"


LIVE = """
import threading, time
from tre import App, Window

window = Window(width=120, height=80)
window.set(transparent=True)
window.root.set(fill=(0, 0, 0, 0))
app = App()
app.add_window(window)
handle = app.thread_handle()
seen = []
def look():
    time.sleep(1.5)
    handle.call_soon(lambda: (seen.append(window.get("transparent_active")),
                              window.set(blur_behind=True), window.close()))
threading.Thread(target=look, daemon=True).start()
try:
    window.set(transparent=False)
except Exception:
    pass
app.run()
print("active", seen[0])
try:
    window.set(transparent=True)
except ValueError as err:
    print("late", "before the window opens" in str(err))
"""


@pytest.mark.skipif(not os.environ.get("DISPLAY"), reason="needs a display")
def test_a_live_transparent_window_reports_whether_its_surface_can_blend():
    script = LIVE.replace("window.set(transparent=False)", "window.set(transparent=True)", 1)
    result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stderr
    out = result.stdout.split()
    assert out[0] == "active" and out[1] in ("True", "False")
