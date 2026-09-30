"""0.5.0 M2 (issue #28, #37): undecorated windows and the window's own
properties and controls -- what a framework drawing its own title bar needs
from `tre`. Everything here runs without a display: a window that isn't open
keeps each setting and applies it when `App.run()` opens it.
"""

import subprocess
import sys
import textwrap

import pytest

from tre import Window


# --- Step 1: decorations -------------------------------------------------------


def test_a_window_is_decorated_by_default():
    assert Window().get("decorations") is True


def test_decorations_can_be_turned_off_at_creation():
    window = Window(width=320, height=200, title="Notes", decorations=False)
    assert window.get("decorations") is False


def test_decorations_can_be_set_either_way():
    window = Window()
    window.set(decorations=False)
    assert window.get("decorations") is False
    window.set(decorations=True)
    assert window.get("decorations") is True


def test_decorations_must_be_a_bool():
    with pytest.raises(ValueError, match="`decorations` must be a bool"):
        Window().set(decorations="no")


def test_the_unknown_property_error_names_decorations():
    with pytest.raises(ValueError, match="decorations"):
        Window().set(colour=1)
    with pytest.raises(ValueError, match="decorations"):
        Window().get("colour")


# --- Step 2: minimize, maximize, restore, close ----------------------------------


def test_controls_before_the_window_opens_do_not_raise():
    window = Window()
    window.maximize()
    window.minimize()
    window.restore()
    window.close()  # nothing is open: nothing to close


def run_live(script: str) -> str:
    """Runs `script` in a fresh interpreter -- a second real `App.run()` in
    this pytest process can break other render-loop tests -- and returns its
    last line of output. Skips where no display is reachable."""
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(script)],
        capture_output=True, text=True, timeout=60,
    )
    assert done.returncode == 0, done.stderr
    last = done.stdout.strip().splitlines()[-1]
    if last == "NO_DISPLAY":
        pytest.skip("no display reachable")
    return last


def test_close_fires_close_requested_first_and_can_be_cancelled():
    log = run_live("""
        from tre import App, Window
        w = Window(width=200, height=120, decorations=False)
        box = w.create("box", width=20, height=20)
        w.root.add_child(box)
        log = []
        def requested(e):
            log.append("requested")
            if log.count("requested") == 1:
                e.cancel()  # the first close is refused
        w.on("close_requested", requested)
        w.on("closed", lambda: log.append("closed"))
        def first():
            log.append("frame")
            w.maximize(); w.minimize(); w.restore()  # live, no errors
            w.close()
            box.animate("opacity", 0.2, 50, on_complete=lambda: w.close())
        box.animate("opacity", 0.5, 50, on_complete=first)
        app = App()
        app.add_window(w)
        # Uncapped in effect: a bounded run renders as fast as it can, so a
        # small cap could end it before the animations do. If close() broke,
        # the subprocess timeout fails the test.
        app.run(max_frames=10_000_000)
        print(log if log else "NO_DISPLAY")
    """)
    assert log == "['frame', 'requested', 'requested', 'closed']", (
        "the first close() is cancelled and the second closes the window"
    )

