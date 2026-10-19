"""0.5.7 (#133): OS file drag and drop on X11, end to end: a real window, a real XDND
drag source (`tools/xdnd_source.py`, a separate X client), and the window's events.

Needs an X display (XWayland counts) and libX11; skips without. The Wayland path is not
covered: winit 0.30 delivers no file events there.
"""

import ctypes.util
import json
import os
import select
import subprocess
import sys
import time
from pathlib import Path

import pytest

ROOT = Path(__file__).parent.parent
TITLE = "tre-file-drop-x11-test"

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or not os.environ.get("DISPLAY") or not ctypes.util.find_library("X11"),
    reason="needs an X display and libX11",
)

CHILD = f"""
import json
from tre import App, Window
window = Window(width=420, height=260, title={TITLE!r})
zone = window.create("box", width=360, height=200, x=30, y=30, position="absolute")
window.root.add_child(zone)
def say(kind, paths=None):
    print(json.dumps({{"kind": kind, "paths": list(paths or [])}}), flush=True)
window.on("file_hover", lambda e: say("hover", e.paths))
window.on("file_hover_cancel", lambda: say("cancel"))
window.on("file_drop", lambda e: say("drop", e.paths))
zone.on("file_drop", lambda e: say("zone_drop", e.paths))
app = App()
app.add_window(window)
app.thread_handle().call_soon(lambda: print("running", flush=True))
app.run()
"""


class Window:
    def __init__(self):
        env = {k: v for k, v in os.environ.items() if k != "WAYLAND_DISPLAY"}
        self.child = subprocess.Popen(
            [sys.executable, "-I", "-c", CHILD], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env,
        )
        self.buffer = b""
        assert self.line(15) == "running"
        time.sleep(0.5)  # let the window map

    def line(self, timeout):
        end = time.time() + timeout
        while b"\n" not in self.buffer:
            assert time.time() < end, "the window printed nothing in time"
            ready, _, _ = select.select([self.child.stdout], [], [], 0.2)
            if ready:
                self.buffer += os.read(self.child.stdout.fileno(), 65536)
        line, _, self.buffer = self.buffer.partition(b"\n")
        return line.decode().strip()

    def events(self, settle=0.6):
        """Every event printed so far, after waiting a moment for the last."""
        time.sleep(settle)
        ready, _, _ = select.select([self.child.stdout], [], [], 0.1)
        if ready:
            self.buffer += os.read(self.child.stdout.fileno(), 65536)
        lines, _, self.buffer = self.buffer.rpartition(b"\n")
        return [json.loads(x) for x in lines.decode().splitlines() if x.startswith("{")]

    def close(self):
        self.child.kill()
        self.child.wait()


@pytest.fixture
def window():
    w = Window()
    yield w
    w.close()


@pytest.fixture
def files(tmp_path):
    paths = []
    for name in ("one.txt", "two.txt"):
        path = tmp_path / name
        path.write_text(name)
        paths.append(str(path))
    return paths


def drag(files, **kwargs):
    sys.path.insert(0, str(ROOT / "tools"))
    import xdnd_source

    return xdnd_source.drag(TITLE, files, **kwargs)


def test_a_multi_file_drop_arrives_as_one_hover_and_one_drop(window, files):
    result = drag(files, points=[(100, 100), (200, 150)])
    assert result["finished"], result
    events = window.events()
    assert [e["kind"] for e in events if e["kind"] != "zone_drop"] == ["hover", "drop"]
    assert [e["paths"] for e in events if e["kind"] in ("hover", "drop")] == [files, files]


def test_a_drag_that_leaves_cancels_the_hover(window, files):
    drag(files, points=[(100, 100)], cancel=True)
    assert [e["kind"] for e in window.events()] == ["hover", "cancel"]


def test_known_limit_the_node_under_the_drag_is_not_found_on_x11(window, files):
    # winit gives a file event no position, so tre uses the pointer's last one; the
    # pointer never entered the window here, so a drop pointed at the zone's middle
    # reaches the window's listener but not the zone's (#133). Delete this test when
    # the position is the drag's own.
    drag(files, points=[(200, 130)])
    kinds = [e["kind"] for e in window.events()]
    assert "drop" in kinds and "zone_drop" not in kinds
