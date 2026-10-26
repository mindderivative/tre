#!/usr/bin/env python3
"""How much GPU memory does a tre window hold? (0.5.8, #207)

    python tools/vram_probe.py                 # a window, two, five, a bar, a full-screen overlay
    python tools/vram_probe.py --output DP-1   # the output the layer windows use (default: the first)

Each scenario runs in its own process on the current Wayland session and reports that
process's video memory ("VRAM") and system memory the GPU maps ("GTT"), read from
`/proc/<pid>/fdinfo`: the amdgpu driver (and a few others that implement the DRM fdinfo memory
keys) counts it per client, so unlike the system-wide `mem_info_vram_used` it is not disturbed
by the compositor or other applications. Without those keys (NVIDIA's proprietary driver, for
one) it prints nothing useful. The windows open on the desktop for about two seconds each.
"""

import argparse
import json
import os
import re
import subprocess
import sys

CHILD = r'''
import json, os, re, sys, threading, time
from tre import App, LayerWindow, Window

def kib(text):
    m = re.match(r"\s*([\d.]+)\s*(KiB|MiB|GiB|B)?", text)
    if not m:
        return 0.0
    return float(m.group(1)) * {"B": 1 / 1024, "KiB": 1, "MiB": 1024, "GiB": 1048576}[m.group(2) or "B"]

def gpu_memory():
    seen, vram, gtt = set(), 0.0, 0.0
    for fd in os.listdir("/proc/self/fdinfo"):
        try:
            text = open("/proc/self/fdinfo/" + fd).read()
        except OSError:
            continue
        if "drm-driver:" not in text:
            continue
        client = re.search(r"drm-client-id:\s*(\d+)", text)
        if (client.group(1) if client else fd) in seen:
            continue
        seen.add(client.group(1) if client else fd)
        v = re.search(r"drm-total-vram:\s*(.+)", text)
        g = re.search(r"drm-total-gtt:\s*(.+)", text)
        vram += kib(v.group(1)) / 1024 if v else 0
        gtt += kib(g.group(1)) / 1024 if g else 0
    return round(vram, 1), round(gtt, 1)

scenario, output = sys.argv[1], sys.argv[2]
app = App()
if scenario == "window":
    w = Window(width=200, height=100); w.root.set(fill=(200, 60, 60, 255)); app.add_window(w)
elif scenario == "two windows":
    for _ in range(2):
        w = Window(width=200, height=100); w.root.set(fill=(200, 60, 60, 255)); app.add_window(w)
elif scenario == "five windows":
    for _ in range(5):
        w = Window(width=200, height=100); w.root.set(fill=(200, 60, 60, 255)); app.add_window(w)
elif scenario == "bar":
    w = LayerWindow("probe", anchor=("top", "left", "right"), size=(0, 32), output=output)
    w.root.set(fill=(200, 60, 60, 255)); app.add_window(w)
else:
    w = LayerWindow("probe", layer="overlay", anchor=("top", "bottom", "left", "right"), size=(0, 0),
                    exclusive_zone=-1, output=output)
    w.root.set(fill=(0, 0, 0, 0)); app.add_window(w)
def sample():
    time.sleep(2.0)
    print(json.dumps(gpu_memory()), flush=True)
    os._exit(0)
threading.Thread(target=sample, daemon=True).start()
app.run()
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--output", default=None, help="connector name for the layer windows")
    args = parser.parse_args()
    output = args.output
    if output is None:
        import tre

        names = [o.name for o in tre.App().outputs()]
        output = names[0] if names else ""
    print(f"{'scenario':<14}{'VRAM MiB':>10}{'GTT MiB':>10}   (output {output or 'none'})")
    for scenario in ("window", "two windows", "five windows", "bar", "overlay"):
        try:
            out = subprocess.run([sys.executable, "-I", "-c", CHILD, scenario, output], capture_output=True,
                                 text=True, timeout=60).stdout.strip().splitlines()
            vram, gtt = json.loads(out[-1])
            print(f"{scenario:<14}{vram:>10.1f}{gtt:>10.1f}")
        except (subprocess.TimeoutExpired, IndexError, ValueError):
            print(f"{scenario:<14}{'no result':>20}")


if __name__ == "__main__":
    main()
