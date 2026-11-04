"""0.6.1 (#260): a callback that queues itself does not outlive the last window.

`LoopHandle.call_soon` callbacks run on every wake while no window is open (0.5.9, #238), and the
queue wakes the loop again after running anything. A callback that queues itself therefore made a
chain of wakes that went on after the loop had been asked to exit; on macOS that kept `run()` from
returning. Linux returned, so the check here is the chain itself: after the last window closes the
callback must not run again.

The loop runs in a child process (a second real `App.run()` in the test process breaks the others)
and the test skips without a display.
"""

import json
import os
import subprocess
import sys
import textwrap

import pytest

needs_display = pytest.mark.skipif(
    not (os.environ.get("WAYLAND_DISPLAY") or os.environ.get("DISPLAY")),
    reason="needs a display for the event loop",
)

CHILD = """
import json, threading, time
from tess_re import App, Window

app = App()
win = Window(width=80, height=60)
app.add_window(win)
handle = app.thread_handle()
state = {"closed": False, "total": 0, "after_closed": 0}
win.on("closed", lambda event=None: state.update(closed=True))


def tick():
    state["total"] += 1
    if state["closed"]:
        state["after_closed"] += 1
    time.sleep(0.005)
    handle.call_soon(tick)


def driver():
    time.sleep(0.5)
    handle.call_soon(tick)
    time.sleep(0.5)
    handle.call_soon(win.close)


threading.Thread(target=driver, daemon=True).start()
started = time.monotonic()
app.run()
print(json.dumps({**state, "seconds": time.monotonic() - started}), flush=True)
"""


@needs_display
def test_a_self_queuing_callback_does_not_run_after_the_last_window_closes():
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(CHILD)],
        capture_output=True, text=True, timeout=40,
    )
    assert done.returncode == 0, done.stderr
    result = json.loads(done.stdout.strip().splitlines()[-1])
    assert result["total"] > 10, "the callback never got going, so the check proves nothing"
    assert result["after_closed"] == 0
    assert result["seconds"] < 5
