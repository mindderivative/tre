"""M87 (tre issue #6): `App.thread_handle()` / `LoopHandle.call_soon`.

A background thread queues a callable; the event-loop thread runs it
inside `App.run()`. This is what lets a file watcher thread drive hot
reload in a live window.

The live-loop cases each run a real, bounded `App.run()` in a fresh
subprocess (the `test_tracing.py` pattern): a second real `App.run()`
in one pytest process can break unrelated render-loop tests. Each
script queues a marker callable *before* `run()`; if it never ran, no
frame ever happened (no display reachable -- `run()` returns
immediately then), and the test skips instead of passing vacuously.

Timing is made deterministic rather than raced: a `max_frames`-bounded
run counts idle frames too, and those take microseconds, so a worker
thread that sleeps even briefly would miss the run entirely.
"""

import subprocess
import sys
import threading

import pytest

from tre import App, LoopHandle, Window

LIVE_SCRIPT = """
import threading
from tre import App, Window

window = Window(width=120, height=80)
rect = window.add_rect(background=(255, 0, 0, 255), width=40, height=40)
app = App()
app.add_window(window)
handle = app.thread_handle()
main_thread = threading.get_ident()
events = []

def from_worker():
    handle.call_soon(lambda: events.append(("worker", threading.get_ident() == main_thread)))
    handle.call_soon(lambda: 1 / 0)
    def change_node():
        rect.animate("opacity", 0.25, duration_ms=0)
        events.append(("after_raise", threading.get_ident() == main_thread))
    handle.call_soon(change_node)

def first_frame():
    events.append(("first_frame", threading.get_ident() == main_thread))
    # Joined here so the run can't finish before the worker has queued
    # anything: a max_frames-bounded run spins idle frames in
    # microseconds. The worker still queues from a genuinely different
    # thread; its callables run on a later frame of this same run.
    worker = threading.Thread(target=from_worker)
    worker.start()
    worker.join()

handle.call_soon(first_frame)
app.run(max_frames=300)

if not events:
    print("NO_FRAMES")
else:
    for name, on_main in events:
        print(f"EVENT {name} {on_main}")
    print(f"OPACITY {rect.get('opacity'):.2f}")
"""


@pytest.fixture(scope="module")
def live_run():
    result = subprocess.run(
        [sys.executable, "-c", LIVE_SCRIPT],
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert result.returncode == 0, result.stderr
    if "NO_FRAMES" in result.stdout:
        pytest.skip("no display reachable -- App.run() rendered no frames")
    events = [
        (line.split()[1], line.split()[2] == "True")
        for line in result.stdout.splitlines()
        if line.startswith("EVENT ")
    ]
    opacity = next(
        float(line.split()[1]) for line in result.stdout.splitlines() if line.startswith("OPACITY ")
    )
    return events, opacity, result.stderr


def test_a_callable_queued_before_run_runs_on_the_first_frame(live_run):
    events, _, _ = live_run
    assert events[0] == ("first_frame", True)


def test_a_callable_queued_from_a_background_thread_runs_on_the_main_thread(live_run):
    events, _, _ = live_run
    assert ("worker", True) in events


def test_callables_run_in_the_order_they_were_queued(live_run):
    events, _, _ = live_run
    names = [name for name, _ in events]
    assert names == ["first_frame", "worker", "after_raise"]


def test_a_raising_callable_is_logged_and_does_not_stop_later_ones(live_run):
    events, _, stderr = live_run
    assert ("after_raise", True) in events
    assert "ZeroDivisionError" in stderr


def test_a_queued_callable_can_change_the_live_tree(live_run):
    _, opacity, _ = live_run
    assert opacity == pytest.approx(0.25)


# --- in-process: no event loop needed ---------------------------------------


def test_thread_handle_returns_a_loop_handle():
    assert isinstance(App().thread_handle(), LoopHandle)


def test_call_soon_rejects_a_non_callable():
    with pytest.raises(TypeError, match="callable"):
        App().thread_handle().call_soon(42)  # type: ignore[arg-type]


def test_call_soon_works_from_a_background_thread_without_raising():
    app = App()
    app.add_window(Window(width=10, height=10))
    handle = app.thread_handle()
    errors = []

    def worker():
        try:
            handle.call_soon(lambda: None)
        except Exception as exc:  # pragma: no cover - the failure path
            errors.append(exc)

    thread = threading.Thread(target=worker)
    thread.start()
    thread.join()
    assert errors == []


def test_app_itself_is_rejected_from_a_background_thread():
    # Why LoopHandle exists: App is single-threaded. Every tre object
    # refuses use from another thread with a PanicException, which derives
    # from BaseException, not Exception (M96: `thread_bound`).
    app = App()
    errors = []

    def worker():
        try:
            app.thread_handle()
        except BaseException as exc:
            errors.append(exc)

    thread = threading.Thread(target=worker)
    thread.start()
    thread.join()
    assert len(errors) == 1
    assert "belongs to the thread that created it" in str(errors[0])
