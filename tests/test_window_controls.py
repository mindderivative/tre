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


# --- Step 4: fullscreen, minimum size, icon, platform ----------------------------


def test_fullscreen_is_set_and_read_back():
    window = Window()
    assert window.get("fullscreen") is False
    window.set(fullscreen=True)
    assert window.get("fullscreen") is True
    with pytest.raises(ValueError, match="`fullscreen` must be a bool"):
        window.set(fullscreen=1)


def test_a_minimum_size_is_set_per_edge():
    window = Window()
    assert (window.get("min_width"), window.get("min_height")) == (0.0, 0.0)
    window.set(min_width=320)
    window.set(min_height=240.5)
    assert (window.get("min_width"), window.get("min_height")) == (320.0, 240.5)


@pytest.mark.parametrize("value", [-1, True, "320", float("inf")])
def test_a_minimum_edge_must_be_a_number_at_least_zero(value):
    with pytest.raises(ValueError, match="`min_width` must be a number >= 0"):
        Window().set(min_width=value)


def test_an_icon_is_rgba_bytes_with_its_size():
    window = Window()
    window.set(icon=(bytes(16 * 16 * 4), 16, 16))
    window.set(icon=None)  # clears it
    with pytest.raises(ValueError, match="16x16 needs 1024 bytes of RGBA8, got 4"):
        window.set(icon=(bytes(4), 16, 16))
    with pytest.raises(ValueError, match="must be \\(rgba, width, height\\)"):
        window.set(icon=(bytes(0), 0, 0))
    with pytest.raises(ValueError, match="must be \\(rgba, width, height\\)"):
        window.set(icon="icon.png")


def test_the_platform_is_one_of_four():
    assert Window().get("platform") in {"wayland", "x11", "windows", "macos"}


def test_state_and_platform_are_read_only():
    for name in ("maximized", "minimized", "active", "platform"):
        with pytest.raises(ValueError, match="read-only"):
            Window().set(**{name: True})


def test_live_fullscreen_and_a_minimum_larger_than_the_window():
    log = run_live("""
        from tre import App, Window
        w = Window(width=300, height=200, decorations=False)
        box = w.create("box", width=20, height=20)
        w.root.add_child(box)
        log = []
        def grown():
            log.append(("min", w.get("width"), w.get("height")))
            w.close()
        def windowed():
            log.append(("fullscreen", w.get("fullscreen")))
            w.set(fullscreen=False, min_width=400, min_height=300)
            box.animate("opacity", 0.8, 400, on_complete=grown)
        def start():
            w.set(fullscreen=True)
            box.animate("opacity", 0.2, 400, on_complete=windowed)
        box.animate("opacity", 0.5, 100, on_complete=start)
        app = App()
        app.add_window(w)
        app.run(max_frames=10_000_000)
        print(log if log else "NO_DISPLAY")
    """)
    assert log == "[('fullscreen', True), ('min', 400.0, 300.0)]", (
        "a minimum larger than the window grows it, whatever the platform does"
    )


def test_the_documented_quiet_state_before_the_window_opens() -> None:
    """The sequence in the Custom Title Bars guide ("Testing without a
    display"): before `App.run()` the controls change the state with no
    event, so simulating the state you are already in fires nothing."""
    window = Window()
    heard = []
    window.on("maximized", lambda e: heard.append(e.maximized))
    window.maximize()
    assert window.get("maximized") is True and heard == []
    window.simulate("maximized", maximized=True)  # no change: no event
    assert heard == []
    window.simulate("maximized", maximized=False)
    window.simulate("maximized", maximized=True)
    assert heard == [False, True]
    fresh = Window()
    fresh.on("maximized", lambda e: heard.append(("fresh", e.maximized)))
    fresh.simulate("maximized", maximized=True)  # a fresh window: it fires
    assert heard[-1] == ("fresh", True)

