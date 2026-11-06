"""0.6.2 (#264, TRE-REQ-012): a node changed in window B during a callback of window A is drawn.

A timer of A (a bar) sets a node of B (a card, another layer surface). Layer surfaces are drawn in an
order that differs from process to process, so when B had already been drawn in that turn, or never
asked for a frame, the change used to wait for the next unrelated wake of the loop (about a second and
a half in the shell's own run); the cases here start several processes, one order each, and read how long
B's next frame took after every change.

Needs a Wayland compositor with the layer shell; skips without one. Run under `tools/sway_live.sh` or
on a real session (two small click-through surfaces are mapped for a couple of seconds).
"""

import json
import subprocess
import sys
import textwrap

import pytest

import tess_re

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or not tess_re.supports("layer_shell"),
    reason="needs a Wayland compositor with the layer-shell protocol",
)

CHILD = """
import json, time
import tess_re

CHANGES, GAP_MS = 5, 150
app = tess_re.App()
handle = app.thread_handle()
a = tess_re.LayerWindow("tre-test-a", layer="top", anchor=("top", "left", "right"), size=(0, 24),
                        keyboard="none", input_region=[])
a.root.set(fill=(30, 30, 40, 255))
b = tess_re.LayerWindow("tre-test-b", layer="overlay", anchor=("top", "left"), size=(120, 60),
                        margin=(40, 0, 0, 20), keyboard="none", input_region=[])
card = b.create("box", width=120, height=60, fill=(10, 10, 10, 255))
b.root.add_child(card)
frames, changes = [], []
b.on("frame", lambda event: frames.append(time.monotonic()))


def change(n):
    changes.append(time.monotonic())
    card.set(fill=(20 + 30 * n, 90, 200, 255))  # always a new colour: a no-op draws nothing


for i in range(CHANGES):
    a.after(500 + i * GAP_MS, lambda n=i: change(n))


def report():
    delays = []
    for t0 in changes:
        later = next((f for f in frames if f >= t0), None)
        delays.append(None if later is None else (later - t0) * 1000)
    print(json.dumps({"delays": delays}), flush=True)
    handle.stop()


a.after(500 + CHANGES * GAP_MS + 600, report)
app.add_window(a)
app.add_window(b)
app.run(keep_alive=True)
"""

LATE_MS = 100  # a draw takes a few ms; the bug was about a whole second


def run_child():
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(CHILD)],
        capture_output=True, text=True, timeout=60, check=False,
    )
    lines = [line for line in done.stdout.splitlines() if line.startswith("{")]
    assert lines, f"the child printed nothing: {done.stderr[-400:]}"
    return json.loads(lines[-1])["delays"]


def test_a_change_to_another_window_made_in_a_timer_is_drawn_promptly():
    # Each process has its own draw order for the two surfaces, so several are needed.
    for run in range(6):
        delays = run_child()
        assert len(delays) == 5
        late = [d for d in delays if d is None or d > LATE_MS]
        assert not late, f"run {run}: delays {delays}"
