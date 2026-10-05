"""0.5.4 (#135): GPU time, cost by node, trace export and a thread-safe reader.

The pieces are unit-tested in Rust (`gpu_timer.rs`, `profile.rs`, `trace.rs`).
Here, the Python surface that needs no display, and live runs in their own
processes (a display is needed to draw frames; they skip without one).
"""

import json
import os
import subprocess
import sys
import textwrap
import threading

import pytest

import tre
from tre import StatsHandle, Window

needs_display = pytest.mark.skipif(not os.environ.get("DISPLAY"), reason="needs a display")


def run(script, timeout=120):
    result = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(script)], capture_output=True, text=True, timeout=timeout
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def test_statistics_say_whether_the_gpu_can_be_timed_and_have_no_profile_yet():
    stats = Window().frame_stats()
    assert stats["gpu_timing"] is False, "the adapter is only known once a window is open"
    assert stats["profile"] is None
    assert stats["recent"]["gpu_ms"] is None


def test_profile_nodes_is_a_window_property():
    window = Window()
    assert window.get("profile_nodes") is False
    window.set(profile_nodes=True)
    assert window.get("profile_nodes") is True
    with pytest.raises(ValueError, match="profile_nodes"):
        window.set(profile_nodes=1)


def test_a_stats_handle_reads_from_any_thread_without_the_loop():
    window = Window()
    handle = window.stats_handle()
    assert isinstance(handle, StatsHandle)
    seen = []
    thread = threading.Thread(target=lambda: seen.append(handle.read()))
    thread.start()
    thread.join(10)
    assert seen and seen[0]["frames"] == 0
    assert seen[0]["profile"] is None
    assert set(handle.read(reset=True)) == set(window.frame_stats())


def test_a_trace_cannot_start_twice_and_stopping_with_none_is_harmless(tmp_path):
    window = Window()
    assert window.stop_trace() == 0
    path = tmp_path / "t.json"
    window.start_trace(str(path))
    with pytest.raises(ValueError, match="already running"):
        window.start_trace(str(tmp_path / "other.json"))
    assert window.stop_trace() == 0, "no frames drawn"
    events = json.loads(path.read_text())
    names = {e["name"] for e in events}
    assert {"process_name", "thread_name"} <= names
    assert window.stop_trace() == 0


def test_a_trace_into_a_missing_directory_raises_oserror(tmp_path):
    with pytest.raises(OSError):
        Window().start_trace(str(tmp_path / "no" / "such" / "dir" / "t.json"))


LIVE = """
import json, sys, threading, time
from tre import App, Window

window = Window(width=240, height=160)
window.set(profile_nodes=True)
card = window.create("box", width=120, height=80, x=60, y=40, position="absolute", fill=(0x67, 0x50, 0xA4, 0xFF), corner_radius=12)
label = window.create("text", text="hello", font_size=18, fill=(255, 255, 255, 255))
card.add_child(label)
window.root.add_child(card)
card.animate("opacity", 0.4, 600000)
handle = window.stats_handle()
window.start_trace(sys.argv[1])
read_from_thread = []
app = App()
app.add_window(window)
loop = app.thread_handle()
def poll():
    # The loop is drawing while this reads: the count it sees grows.
    for _ in range(40):
        time.sleep(0.02)
        read_from_thread.append(handle.read()["frames"])
threading.Thread(target=poll, daemon=True).start()
app.run(max_frames=120)
frames = window.stop_trace()
stats = window.frame_stats()
profile = stats["profile"]
print(json.dumps({
    "gpu_timing": stats["gpu_timing"],
    "recent_gpu": stats["recent"]["gpu_ms"],
    "frames": stats["frames"],
    "traced": frames,
    "profile_kinds": sorted(profile["by_kind"]) if profile else None,
    "profile_has_nodes": bool(profile and profile["slowest"] and profile["slowest"][0]["node"] is not None),
    "thread_reads": read_from_thread,
}))
"""


@needs_display
def test_a_live_window_reports_gpu_time_node_costs_a_trace_and_a_threaded_read(tmp_path):
    path = tmp_path / "live.json"
    out = json.loads(run(LIVE.replace("sys.argv[1]", repr(str(path)))).strip().splitlines()[-1])
    assert out["frames"] >= 30
    if out["gpu_timing"]:
        assert out["recent_gpu"] is not None and out["recent_gpu"] >= 0.0
    else:
        assert out["recent_gpu"] is None
    assert out["profile_kinds"] and {"box", "text"} <= set(out["profile_kinds"]) | {"container"}
    assert out["profile_has_nodes"]
    reads = out["thread_reads"]
    assert reads and reads == sorted(reads) and reads[-1] > reads[0], f"counts seen from the thread: {reads}"
    # The trace holds the frames drawn after it started, and loads as JSON.
    assert 0 < out["traced"] <= out["frames"]
    events = json.loads(path.read_text())
    frames = [e for e in events if e["name"].startswith("frame ")]
    assert len(frames) == out["traced"]
    assert {"draw", "layout"} <= {e["name"] for e in events} or {"draw"} <= {e["name"] for e in events}
    for e in frames:
        assert e["ph"] == "X" and e["dur"] > 0 and e["tid"] == 1
    stamps = [e["ts"] for e in frames]
    assert stamps == sorted(stamps)
    if out["gpu_timing"]:
        assert any(e["name"].startswith("gpu frame") and e["tid"] == 2 for e in events)
