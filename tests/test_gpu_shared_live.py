"""0.5.9 (#220): windows draw with one GPU device, not one each.

A window used to make its own `wgpu` instance, adapter and device; after the small memory blocks
(#219) every further window still held about 13 MiB of video memory and 11 MiB of mappable memory,
plus the time to create them. Now the first window's handles are shared with the others
(`crates/engine-py/src/gpu_share.rs`), so five windows hold about what one does.
`TRE_GPU_DEVICES=per-window` gives each window its own again.

Each case runs the app in a child process and reads that process's own GPU memory from
`/proc/self/fdinfo` (`drm-total-vram`, which amdgpu and a few other drivers report per client). The
tests skip without a display or without those keys; the loss test needs only a display. Windows
open on the desktop for a few seconds, as in the other live tests, so run them under a private
compositor (`tools/sway_live.sh`) when you would rather not see them.
"""

import json
import os
import subprocess
import sys

import pytest

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or not (os.environ.get("WAYLAND_DISPLAY") or os.environ.get("DISPLAY")),
    reason="needs a display",
)

MEMORY = r"""
import json, os, re, threading, time
from tre import App, Window

def mib(text):
    m = re.match(r"\s*([\d.]+)\s*(KiB|MiB|GiB|B)?", text)
    return float(m.group(1)) * {"B": 1 / 1048576, "KiB": 1 / 1024, "MiB": 1, "GiB": 1024}[m.group(2) or "B"] if m else 0.0

def vram():
    seen, total = set(), 0.0
    for fd in os.listdir("/proc/self/fdinfo"):
        try:
            text = open("/proc/self/fdinfo/" + fd).read()
        except OSError:
            continue
        if "drm-driver:" not in text:
            continue
        client = re.search(r"drm-client-id:\s*(\d+)", text)
        key = client.group(1) if client else fd
        if key in seen:
            continue
        seen.add(key)
        v = re.search(r"drm-total-vram:\s*(.+)", text)
        total += mib(v.group(1)) if v else 0.0
    return total if seen else None

app = App()
windows = []
for _ in range(int(os.environ["TRE_TEST_WINDOWS"])):
    w = Window(width=200, height=100)
    w.root.set(fill=(200, 60, 60, 255))
    app.add_window(w)
    windows.append(w)
handle = app.thread_handle()

def report():
    time.sleep(2.5)
    total = vram()
    stats = []
    done = threading.Event()
    def read():
        for w in windows:
            s = w.frame_stats()
            stats.append({"frames": s["frames"], "memory": s["memory"]})
        done.set()
    handle.call_soon(read)
    done.wait(5)
    print(json.dumps({"vram": total, "stats": stats}), flush=True)
    os._exit(0)
threading.Thread(target=report, daemon=True).start()
app.run()
"""


def run_memory(windows, **env):
    out = subprocess.run(
        [sys.executable, "-I", "-c", MEMORY], capture_output=True, text=True, timeout=90,
        env=dict(os.environ, TRE_TEST_WINDOWS=str(windows), **env),
    ).stdout.strip().splitlines()
    lines = [line for line in out if line.startswith("{")]
    assert lines, "the windows printed nothing"
    return json.loads(lines[-1])


@pytest.fixture(scope="module")
def reading():
    one = run_memory(1)
    # A real GPU device holds at least ~18 MiB here; a software adapter shows a few MiB of some
    # unrelated handle, which would make the comparisons below mean nothing.
    if one["vram"] is None or one["vram"] < 10:
        pytest.skip("this adapter reports no per-process video memory for its device in fdinfo")
    return one


def test_five_windows_hold_about_what_one_window_does(reading):
    five = run_memory(5)
    assert five["vram"] - reading["vram"] < 12, f"one {reading['vram']} MiB, five {five['vram']} MiB"


def test_a_window_each_is_one_environment_variable_away(reading):
    five = run_memory(5, TRE_GPU_DEVICES="per-window")
    assert five["vram"] - reading["vram"] > 30, f"one {reading['vram']} MiB, five {five['vram']} MiB"


def test_every_window_draws_and_reads_the_devices_memory(reading):
    five = run_memory(5)
    assert len(five["stats"]) == 5
    assert all(s["frames"] >= 1 for s in five["stats"]), "each window drew"
    memories = [s["memory"] for s in five["stats"]]
    assert all(m is not None and m["shared_device"] is True for m in memories)
    assert len({m["reserved_bytes"] for m in memories}) == 1, "one device: every window reads the same reserved bytes"
    own = run_memory(2, TRE_GPU_DEVICES="per-window")
    assert all(s["memory"]["shared_device"] is False for s in own["stats"])


def allocated_mib(result):
    memory = result["stats"][0]["memory"]
    return None if memory is None else memory["allocated_bytes"] / 1048576


def test_windows_share_one_vello_renderer_so_the_device_holds_no_more_for_thirty_than_for_one():
    """(#235) A renderer of its own cost each window about 0.2 MiB of device memory (and 4 MiB of
    process memory and 9 ms); shared, thirty windows use what one does. Read from the device's
    allocator, so it needs no per-process driver keys."""
    one = allocated_mib(run_memory(1))
    if one is None:
        pytest.skip("this backend has no allocator report")
    thirty = allocated_mib(run_memory(30))
    assert thirty - one < 1.5, f"one {one:.2f} MiB, thirty {thirty:.2f} MiB"


def test_a_renderer_each_is_one_environment_variable_away():
    one = allocated_mib(run_memory(1))
    if one is None:
        pytest.skip("this backend has no allocator report")
    thirty = allocated_mib(run_memory(30, TRE_GPU_RENDERERS="per-window"))
    assert thirty - one > 3, f"one {one:.2f} MiB, thirty {thirty:.2f} MiB: windows still share a renderer"


LOSS = r"""
import json, os, threading, time
from tre import App, Window

app = App()
heard = {}
windows, boxes = {}, {}
for name in ("a", "b", "c"):
    w = Window(width=160, height=100)
    box = w.create("box", width=20, height=20)
    w.root.add_child(box)
    box.animate("scale", 1.1, 60000)  # keeps frames running until the loss
    w.on("gpu_lost", lambda e, name=name: heard.setdefault(name, e.reason))
    windows[name], boxes[name] = w, box
    app.add_window(w)
boxes["a"].animate("opacity", 0.5, 400, on_complete=lambda: windows["a"]._lose_gpu())
try:
    app.run(max_frames=None)
    print(json.dumps({"returned": True, "heard": heard}))
except RuntimeError as err:
    print(json.dumps({"error": str(err).split(";")[0], "heard": heard}))
"""


def test_a_lost_shared_device_is_heard_by_every_window_and_ends_the_run():
    """One window loses the device; the other two share it, so all three hear `gpu_lost` and
    `App.run()` raises once."""
    done = subprocess.run([sys.executable, "-I", "-c", LOSS], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    result = json.loads(done.stdout.strip().splitlines()[-1])
    if result.get("returned"):
        pytest.skip("no display reachable")
    assert result["error"].startswith("the GPU was lost: the device was destroyed"), result
    assert result["heard"] == {"a": "destroyed", "b": "destroyed", "c": "destroyed"}, result
