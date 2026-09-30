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



# --- Step 3: state and its events ------------------------------------------------


def test_a_window_starts_unmaximized_unminimized_and_inactive():
    window = Window()
    assert (window.get("maximized"), window.get("minimized"), window.get("active")) == (
        False, False, False,
    )


def test_before_the_window_opens_controls_set_how_it_opens():
    window = Window()
    window.maximize()
    window.minimize()
    assert (window.get("maximized"), window.get("minimized")) == (True, True)
    window.restore()
    assert (window.get("maximized"), window.get("minimized")) == (False, False)


def test_simulated_state_fires_its_event_only_when_it_changes():
    window = Window()
    heard = []
    window.on("maximized", lambda e: heard.append(("maximized", e.maximized, e.type)))
    window.on("active", lambda e: heard.append(("active", e.active, e.type)))
    window.simulate("maximized", maximized=True)
    window.simulate("maximized", maximized=True)  # no change: no event
    window.simulate("active", active=True)
    window.simulate("maximized", maximized=False)
    window.simulate("active", active=False)
    assert heard == [
        ("maximized", True, "maximized"),
        ("active", True, "active"),
        ("maximized", False, "maximized"),
        ("active", False, "active"),
    ]
    assert (window.get("maximized"), window.get("active")) == (False, False)


def test_simulating_state_needs_its_value():
    with pytest.raises(ValueError, match="needs `maximized`"):
        Window().simulate("maximized")
    with pytest.raises(ValueError, match="`active` must be a bool"):
        Window().simulate("active", active="yes")


def test_a_live_maximize_and_restore_fire_maximized():
    log = run_live("""
        from tre import App, Window
        w = Window(width=300, height=200, decorations=False)
        box = w.create("box", width=20, height=20)
        w.root.add_child(box)
        log = []
        w.on("maximized", lambda e: log.append(("event", e.maximized)))
        def restore():
            log.append(("get", w.get("maximized")))
            w.restore()
            box.animate("opacity", 0.9, 400, on_complete=lambda: w.close())
        def maximize():
            w.maximize()
            box.animate("opacity", 0.2, 400, on_complete=restore)
        box.animate("opacity", 0.5, 100, on_complete=maximize)
        app = App()
        app.add_window(w)
        app.run(max_frames=10_000_000)
        print(log if log else "NO_DISPLAY")
    """)
    assert log == "[('event', True), ('get', True), ('event', False)]"
