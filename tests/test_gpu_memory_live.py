"""0.5.7 (#219): a window's GPU device reserves small memory blocks, not 192 MiB.

`wgpu`'s default made device memory blocks of at least 128 MiB the moment a window's device
existed, so a 200 x 100 window held about 210 MiB of video memory (`docs/design/vram-per-window.md`).
tre now asks for small blocks; `TRE_GPU_MEMORY=performance` brings the old reservation back.

The window runs in its own process and reads that process's own GPU memory from
`/proc/self/fdinfo` (`drm-total-vram`, which amdgpu and a few other drivers report per client). The
test skips without a display or without those keys. A window opens on the desktop for a few
seconds, as in the other live tests.
"""

import json
import os
import re
import subprocess
import sys

import pytest

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or not (os.environ.get("WAYLAND_DISPLAY") or os.environ.get("DISPLAY")),
    reason="needs a display and a Linux DRM driver that reports per-process memory",
)

CHILD = r"""
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
w = Window(width=200, height=100)
w.root.set(fill=(200, 60, 60, 255))
app.add_window(w)
def report():
    time.sleep(2.0)
    print(json.dumps({"vram": vram()}), flush=True)
    os._exit(0)
threading.Thread(target=report, daemon=True).start()
app.run()
"""


def window_vram(**env):
    out = subprocess.run(
        [sys.executable, "-I", "-c", CHILD], capture_output=True, text=True, timeout=60,
        env=dict(os.environ, **env),
    ).stdout.strip().splitlines()
    lines = [line for line in out if line.startswith("{")]
    assert lines, "the window printed nothing"
    return json.loads(lines[-1])["vram"]


def test_a_small_window_holds_a_few_tens_of_mib_not_two_hundred():
    vram = window_vram()
    if vram is None or vram == 0:
        pytest.skip("this driver reports no per-process video memory in fdinfo")
    assert vram < 100, f"{vram} MiB: the device reserves big blocks again"


def test_the_environment_variable_brings_the_big_reservation_back():
    small = window_vram()
    if small is None or small == 0:
        pytest.skip("this driver reports no per-process video memory in fdinfo")
    big = window_vram(TRE_GPU_MEMORY="performance")
    assert big > small + 100, f"performance {big} MiB vs default {small} MiB"
