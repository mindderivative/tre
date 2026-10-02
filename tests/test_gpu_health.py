"""0.5.1 (#65): what the GPU tells a window about its own health -- `gpu_lost`,
`gpu_error`, `gpu_stalled`, and the opt-in `gpu_watchdog`. The detection
itself (polling, the lost callback, the error handler, the watchdog's
timing) is tested in `engine-render`; here, the Python surface, and one
live test that really loses a window's GPU.
"""

import subprocess
import sys
import textwrap

import pytest

from tre import Window


# --- the watchdog property -------------------------------------------------------


def test_the_watchdog_is_off_by_default_and_set_in_seconds():
    window = Window()
    assert window.get("gpu_watchdog") is None
    window.set(gpu_watchdog=5)
    assert window.get("gpu_watchdog") == 5.0
    window.set(gpu_watchdog=0.25)
    assert window.get("gpu_watchdog") == 0.25
    window.set(gpu_watchdog=None)
    assert window.get("gpu_watchdog") is None


@pytest.mark.parametrize("value", [0, -1, "5", True, float("nan"), float("inf")])
def test_the_watchdog_must_be_positive_seconds_or_none(value):
    window = Window()
    with pytest.raises(ValueError, match="`gpu_watchdog` must be a number of seconds > 0, or None"):
        window.set(gpu_watchdog=value)
    assert window.get("gpu_watchdog") is None, "a failed set changes nothing"


def test_the_property_lists_name_the_watchdog():
    with pytest.raises(ValueError, match="gpu_watchdog"):
        Window().set(colour=1)
    with pytest.raises(ValueError, match="gpu_watchdog"):
        Window().get("colour")


# --- the events, simulated -------------------------------------------------------


def test_simulated_gpu_events_reach_their_listeners():
    window = Window()
    heard = []
    window.on("gpu_lost", lambda e: heard.append(("lost", e.type, e.reason, e.message)))
    window.on("gpu_error", lambda e: heard.append(("error", e.type, e.message)))
    window.on("gpu_stalled", lambda e: heard.append(("stalled", e.type, e.seconds)))
    window.simulate("gpu_lost", reason="destroyed", message="")
    window.simulate("gpu_lost", message="device hung")  # a fault by default
    window.simulate("gpu_error", message="Validation Error in create_buffer")
    window.simulate("gpu_stalled", seconds=12.5)
    assert heard == [
        ("lost", "gpu_lost", "destroyed", ""),
        ("lost", "gpu_lost", "unknown", "device hung"),
        ("error", "gpu_error", "Validation Error in create_buffer"),
        ("stalled", "gpu_stalled", 12.5),
    ]


def test_simulated_gpu_events_need_their_payload():
    window = Window()
    with pytest.raises(ValueError, match="needs `message`"):
        window.simulate("gpu_error")
    with pytest.raises(ValueError, match="needs `seconds`"):
        window.simulate("gpu_stalled")
    with pytest.raises(ValueError, match="`reason` must be a str"):
        window.simulate("gpu_lost", reason=1)


def test_the_unknown_event_errors_name_the_gpu_events():
    with pytest.raises(ValueError, match="gpu_lost"):
        Window().on("nonsense", lambda: None)
    with pytest.raises(ValueError, match="gpu_lost"):
        Window().simulate("nonsense")


# --- a real lost GPU --------------------------------------------------------------


def run_live(script: str) -> str:
    """Runs `script` in a fresh interpreter -- a second real `App.run()` in
    this pytest process can break other render-loop tests -- and returns its
    last line of output."""
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(script)],
        capture_output=True, text=True, timeout=60,
    )
    assert done.returncode == 0, done.stderr
    return done.stdout.strip().splitlines()[-1]


def test_a_lost_gpu_fires_gpu_lost_and_ends_the_run_with_an_error():
    """The device is destroyed from inside a running window (a private test
    hook), which the loop's poll then reports as lost: the listener hears
    `gpu_lost`, a `close_requested` listener that tries to keep the window
    open can't, and `App.run()` raises instead of freezing or panicking. If
    the loop did not end, the subprocess timeout would fail this test."""
    out = run_live("""
        from tre import App, Window
        w = Window(width=200, height=120)
        box = w.create("box", width=20, height=20)
        w.root.add_child(box)
        heard = []
        w.on("gpu_lost", lambda e: heard.append((e.reason, e.message)))
        w.on("close_requested", lambda e: (heard.append("tried to stay"), e.cancel()))
        box.animate("opacity", 0.5, 100, on_complete=lambda: w._lose_gpu())
        box.animate("scale", 1.1, 60000)  # keeps frames running until the loss
        app = App()
        app.add_window(w)
        try:
            app.run(max_frames=None)
            print("RETURNED", heard)
        except RuntimeError as e:
            print("RAISED", str(e).split(";")[0], heard)
    """)
    if out == "RETURNED []":
        pytest.skip("no display reachable")
    assert out.startswith("RAISED the GPU was lost: the device was destroyed"), out
    assert "('destroyed', '')" in out, "the listener heard gpu_lost"
    assert "tried to stay" not in out, "a close_requested listener can't keep the window open"
