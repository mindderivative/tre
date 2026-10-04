"""0.5.4 (#140): `tre.CursorImage`, a pointer shape drawn from pixels.

Validation and the Python surface are tested here; that the OS cursor is built
needs a live event loop, so one test runs a window in its own process.
"""

import os
import subprocess
import sys

import pytest

import tre
from tre import CursorImage, Window


def pixels(width=8, height=8, value=200):
    return bytes([value]) * (width * height * 4)


def test_a_cursor_image_reports_its_size_and_hotspot():
    image = CursorImage(pixels(12, 10), 12, 10, hotspot=(3, 2))
    assert image.size == (12, 10)
    assert image.hotspot == (3, 2)
    assert image.ready is False
    assert "12x10" in repr(image)


def test_the_hotspot_defaults_to_the_top_left_corner():
    assert CursorImage(pixels(), 8, 8).hotspot == (0, 0)


def test_the_same_image_is_equal_and_a_different_one_is_not():
    a = CursorImage(pixels(), 8, 8, hotspot=(1, 1))
    assert a == CursorImage(pixels(), 8, 8, hotspot=(1, 1))
    assert hash(a) == hash(CursorImage(pixels(), 8, 8, hotspot=(1, 1)))
    assert a != CursorImage(pixels(), 8, 8, hotspot=(2, 1))
    assert a != CursorImage(pixels(value=90), 8, 8, hotspot=(1, 1))


@pytest.mark.parametrize(
    "args, message",
    [
        ((b"\x00" * 10, 8, 8), "needs 256 bytes"),
        ((pixels(), 0, 8), "1 to 256"),
        ((pixels(), 8, 300), "1 to 256"),
        ((pixels(), 70000, 8), "width must be 1 to 256"),
    ],
)
def test_a_bad_image_is_a_value_error_saying_why(args, message):
    with pytest.raises(ValueError, match=message):
        CursorImage(*args)


def test_a_hotspot_outside_the_image_is_refused():
    with pytest.raises(ValueError, match="hotspot"):
        CursorImage(pixels(), 8, 8, hotspot=(8, 0))
    with pytest.raises(ValueError, match="hotspot"):
        CursorImage(pixels(), 8, 8, hotspot=(0, 99))


def test_a_node_takes_a_cursor_image_and_reads_it_back():
    window = Window()
    box = window.create("box", width=20, height=20)
    image = CursorImage(pixels(), 8, 8, hotspot=(2, 2))
    box.set(cursor=image)
    got = box.get("cursor")
    assert isinstance(got, CursorImage) and got == image
    assert got.size == (8, 8) and got.hotspot == (2, 2)
    box.set(cursor="pointer")
    assert box.get("cursor") == "pointer"
    box.set(cursor=image)
    box.set(cursor=None)
    assert box.get("cursor") is None


def test_a_bad_cursor_value_still_names_what_is_accepted():
    box = Window().create("box", width=20, height=20)
    with pytest.raises(ValueError, match="CursorImage"):
        box.set(cursor=3)
    with pytest.raises(ValueError, match="one of"):
        box.set(cursor="hand")


def test_descendants_inherit_a_cursor_image_like_any_cursor():
    window = Window(width=100, height=100)
    outer = window.create("box", width=80, height=80)
    inner = window.create("box", width=20, height=20)
    window.root.add_child(outer)
    outer.add_child(inner)
    outer.set(cursor=CursorImage(pixels(), 8, 8))
    assert inner.get("cursor") is None, "inherited at the pointer, not copied onto children"


def test_it_is_exported():
    assert tre.CursorImage is CursorImage


LIVE = """
import threading, time
from tre import App, CursorImage, Window

window = Window(width=120, height=80)
image = CursorImage(bytes([255, 0, 0, 255]) * (16 * 16), 16, 16, hotspot=(8, 8))
window.root.set(cursor=image)
before = image.ready
app = App()
app.add_window(window)
handle = app.thread_handle()
threading.Thread(target=lambda: (time.sleep(1.5), handle.call_soon(window.close)), daemon=True).start()
app.run()
print(before, image.ready)
"""


@pytest.mark.skipif(not os.environ.get("DISPLAY"), reason="needs a display")
def test_a_live_loop_builds_the_os_cursor():
    result = subprocess.run([sys.executable, "-c", LIVE], capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stderr
    assert result.stdout.split()[-2:] == ["False", "True"]
