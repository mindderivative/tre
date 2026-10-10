"""0.5.6 (#163): `Window.after`, `Window.every` and `TimerHandle`.

Headless cases move the clock with `advance`; the live case runs in its own
process (it needs a display, and skips without one).
"""

import os
import subprocess
import sys
import textwrap

import pytest

from tre import TimerHandle, Window


def test_after_fires_once_at_its_time():
    window = Window()
    log = []
    handle = window.after(100, lambda: log.append("a"))
    assert isinstance(handle, TimerHandle) and handle.active
    window.advance(99)
    assert log == [] and handle.active
    window.advance(1)
    assert log == ["a"] and not handle.active
    window.advance(1000)
    assert log == ["a"]


def test_every_runs_on_its_beat_inside_one_advance():
    window = Window()
    count = []
    handle = window.every(100, lambda: count.append(1))
    window.advance(1000)
    assert len(count) == 10 and handle.active
    window.advance(250)
    assert len(count) == 12


def test_timers_run_in_order_of_their_time():
    window = Window()
    log = []
    window.after(300, lambda: log.append("c"))
    window.after(100, lambda: log.append("a"))
    window.after(200, lambda: log.append("b"))
    window.advance(500)
    assert log == ["a", "b", "c"]


def test_cancel_stops_a_timer_and_reports_whether_it_was_pending():
    window = Window()
    log = []
    once = window.after(100, lambda: log.append("once"))
    beat = window.every(50, lambda: log.append("beat"))
    assert once.cancel() is True
    assert once.cancel() is False
    window.advance(120)
    assert log == ["beat", "beat"]
    beat.cancel()
    window.advance(500)
    assert log == ["beat", "beat"] and not beat.active


def test_a_callback_can_cancel_another_timer_due_at_the_same_time():
    window = Window()
    log = []
    holder = {}
    window.after(100, lambda: (log.append("first"), holder["b"].cancel()))
    holder["b"] = window.after(100, lambda: log.append("second"))
    window.advance(200)
    assert log == ["first"]


def test_a_callback_can_start_a_timer_and_it_counts_from_when_it_ran():
    window = Window()
    log = []
    window.after(100, lambda: window.after(100, lambda: log.append("chained")))
    window.advance(150)
    assert log == []
    window.advance(60)
    assert log == ["chained"]


def test_an_every_that_raises_keeps_going():
    window = Window()
    count = []

    def boom():
        count.append(1)
        raise RuntimeError("boom")

    window.every(100, boom)
    window.advance(350)
    assert len(count) == 3


def test_timers_can_set_window_properties_while_advancing():
    window = Window()
    window.after(10, lambda: window.set(title="done"))
    window.advance(20)
    assert window.get("title") == "done"


def test_bad_arguments_are_rejected():
    window = Window()
    with pytest.raises(ValueError, match="callable"):
        window.after(10, 5)
    with pytest.raises(ValueError, match="ms must be"):
        window.after(-1, lambda: None)
    with pytest.raises(ValueError, match="ms must be"):
        window.every(0, lambda: None)
    with pytest.raises(ValueError, match="ms must be"):
        window.after(float("nan"), lambda: None)


def test_a_timer_that_refers_to_its_window_does_not_leak_it():
    import gc
    import weakref

    class Sentinel:
        pass

    window = Window()
    sentinel = Sentinel()
    ref = weakref.ref(sentinel)
    # a cycle: window -> timer -> callback -> window and sentinel
    window.after(1000, lambda: (window, sentinel))
    del window, sentinel
    gc.collect()
    assert ref() is None


LIVE = """
import json
from tre import App, Window

window = Window(width=200, height=120)
box = window.create("box", width=60, height=40, fill=(255, 0, 0, 255))
window.root.add_child(box)
log = []
app = App()
app.add_window(window)
t0 = {}

def started(event):
    import time
    t0["at"] = time.perf_counter()

window.on("frame", lambda e: t0.setdefault("first", True) and None)

import time
start = time.perf_counter()
ticks = []
def tick():
    ticks.append(time.perf_counter() - start)
    if len(ticks) == 3:
        window.after(50, window.close)

window.every(200, tick)
app.run()
stats = window.frame_stats()
print(json.dumps({"ticks": ticks, "frames": stats["frames"]}))
"""


@pytest.mark.skipif(not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")),
                    reason="needs a display")
def test_an_idle_window_sleeps_between_timers_and_wakes_for_each():
    import json

    result = subprocess.run([sys.executable, "-c", LIVE], capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout.strip().splitlines()[-1])
    ticks = data["ticks"]
    assert len(ticks) == 3
    # on a 200 ms beat, each within a frame or two of its time
    for index, at in enumerate(ticks, start=1):
        assert 0.2 * index - 0.05 <= at <= 0.2 * index + 0.4, ticks
    # about 0.65 s of run: an idle window redraws a few times, not 40
    assert data["frames"] < 25, data
