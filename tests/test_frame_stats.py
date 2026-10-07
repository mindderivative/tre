"""0.5.4 (#116): `window.frame_stats()` and the window's `frame` event.

The aggregation is unit-tested in Rust (`frame_stats.rs`). Here, the shape of
what Python gets, and a live run in its own process (it needs a display, and
skips without one).
"""

import os
import subprocess
import sys
import textwrap

import pytest

from tre import Window

STAGES = {"tick", "layout", "configure", "prepare", "acquire", "draw", "present"}


def test_a_window_that_never_drew_has_empty_statistics():
    stats = Window().frame_stats()
    assert stats["frames"] == 0 and stats["skipped"] == 0
    assert stats["last"] is None
    recent = stats["recent"]
    assert recent["count"] == 0 and recent["fps"] == 0.0
    assert set(recent["stage_ms"]) == STAGES
    assert set(recent["total_ms"]) == {"mean", "p95", "max"}
    assert set(recent["cpu_ms"]) == {"mean", "p95", "max"}
    assert recent["redraws"] == {"nothing": 0, "full": 0, "partial": 0}


def test_reset_is_accepted_and_the_frame_event_is_a_valid_name():
    window = Window()
    window.frame_stats(reset=True)
    window.on("frame", lambda e: None)
    with pytest.raises(ValueError, match="frame"):
        window.on("nonsense", lambda: None)


LIVE = """
import json, sys
from tre import App, Window

window = Window(width=200, height=120)
box = window.create("box", width=60, height=40, fill=(255, 0, 0, 255))
window.root.add_child(box)
box.animate("opacity", 0.3, 600000)          # something animating: frames keep coming
events = []
window.on("frame", lambda e: events.append(e.stats))
app = App()
app.add_window(window)
app.run(max_frames=40)
stats = window.frame_stats()
print(json.dumps({"stats": stats, "events": len(events), "first_event": events[0] if events else None}))
window.frame_stats(reset=True)
print(json.dumps(window.frame_stats()["frames"]))
"""


@pytest.mark.skipif(not os.environ.get("DISPLAY"), reason="needs a display")
def test_a_live_window_reports_its_frames():
    import json

    result = subprocess.run([sys.executable, "-c", LIVE], capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stderr
    lines = result.stdout.strip().splitlines()
    data = json.loads(lines[-2])
    stats = data["stats"]
    assert stats["frames"] >= 20, stats["frames"]
    last = stats["last"]
    assert last["frame"] == stats["frames"]
    assert last["nodes"] >= 2 and last["width"] > 0 and last["height"] > 0
    assert last["redraw"] in ("nothing", "full", "partial")
    for key in ("tick_ms", "layout_ms", "configure_ms", "prepare_ms", "acquire_ms", "draw_ms",
                "present_ms", "total_ms", "cpu_ms"):
        assert last[key] >= 0.0, (key, last)
    # The stages fit inside the whole, and the app's own cost leaves out the waits.
    parts = sum(last[k] for k in ("tick_ms", "layout_ms", "configure_ms", "prepare_ms",
                                   "acquire_ms", "draw_ms", "present_ms"))
    assert parts <= last["total_ms"] + 1.0
    assert last["cpu_ms"] <= last["total_ms"] + 1e-6
    recent = stats["recent"]
    assert recent["count"] >= 20
    assert recent["total_ms"]["max"] >= recent["total_ms"]["p95"] >= 0
    assert recent["total_ms"]["max"] >= recent["total_ms"]["mean"]
    assert sum(recent["redraws"].values()) == recent["count"]
    # The `frame` event fired for every frame drawn, carrying the same numbers.
    assert data["events"] == stats["frames"]
    assert set(data["first_event"]) == set(last)
    # reset cleared the history.
    assert json.loads(lines[-1]) == 0
