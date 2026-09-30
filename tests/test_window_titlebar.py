"""0.5.0 M4 (issue #28, #39): macOS's overlay title bar, as a framework
sees it on any platform -- `titlebar_inset`, `native_controls`, and the
`titlebar_inset` event. On macOS an undecorated window keeps its traffic
lights over the content; everywhere else there are no OS controls to leave
room for, so the inset is 0. The macOS side itself is compiled here and
checked by hand (M5).
"""

import ast
import subprocess
import sys
import textwrap

import pytest

from tre import Window

MACOS = Window().get("platform") == "macos"


def test_a_decorated_window_has_no_inset_and_no_native_controls():
    window = Window()
    assert window.get("titlebar_inset") == (0.0, 0.0)
    assert window.get("native_controls") is False


def test_native_controls_follow_decorations_and_fullscreen():
    window = Window(decorations=False)
    assert window.get("native_controls") is MACOS, "the traffic lights, on macOS only"
    window.set(fullscreen=True)
    assert window.get("native_controls") is False, "hidden in fullscreen"


def test_inset_and_native_controls_are_read_only():
    for name in ("titlebar_inset", "native_controls"):
        with pytest.raises(ValueError, match="read-only"):
            Window().set(**{name: 0})


def test_the_unknown_property_error_names_them():
    with pytest.raises(ValueError, match="titlebar_inset, native_controls"):
        Window().get("colour")


def test_simulated_inset_fires_its_event_only_when_it_changes():
    window = Window(decorations=False)
    heard = []
    window.on("titlebar_inset", lambda e: heard.append((e.type, e.titlebar_inset)))
    window.simulate("titlebar_inset", height=28, width=78)
    window.simulate("titlebar_inset", height=28, width=78)  # no change: no event
    window.simulate("titlebar_inset", height=0, width=0)  # fullscreen, say
    assert heard == [
        ("titlebar_inset", (28.0, 78.0)),
        ("titlebar_inset", (0.0, 0.0)),
    ]


def test_before_the_window_opens_the_inset_is_the_last_reported():
    window = Window(decorations=False)
    window.simulate("titlebar_inset", height=28, width=78)
    assert window.get("titlebar_inset") == (28.0, 78.0)


def test_simulating_the_inset_needs_both_edges():
    with pytest.raises(ValueError, match="needs `width`"):
        Window().simulate("titlebar_inset", height=28)


def test_live_decorations_and_inset():
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent("""
            from tre import App, Window
            w = Window(width=300, height=200)
            box = w.create("box", width=20, height=20)
            w.root.add_child(box)
            log = []
            def check():
                log.append((w.get("decorations"), w.get("titlebar_inset"),
                            w.get("native_controls")))
                w.close()
            def undecorate():
                w.set(decorations=False)
                box.animate("opacity", 0.2, 300, on_complete=check)
            box.animate("opacity", 0.5, 100, on_complete=undecorate)
            app = App()
            app.add_window(w)
            app.run(max_frames=10_000_000)
            print(log if log else "NO_DISPLAY")
        """)],
        capture_output=True, text=True, timeout=60,
    )
    assert done.returncode == 0, done.stderr
    last = done.stdout.strip().splitlines()[-1]
    if last == "NO_DISPLAY":
        pytest.skip("no display reachable")
    decorations, inset, native = ast.literal_eval(last)[0]
    assert decorations is False
    assert native is MACOS
    assert (inset != (0.0, 0.0)) is MACOS, "the traffic lights' area, on macOS only"


def test_the_documented_layout_follows_the_inset():
    """docs/api/python/window.md's example: the bar clears the traffic
    lights and hides its own buttons when the OS shows them."""
    window = Window(decorations=False)
    bar = window.create("box", height=40, window_region="drag")
    own_buttons = [window.create("box", width=30, height=30) for _ in range(3)]

    def lay_out_bar(inset=None):
        height, width = inset or window.get("titlebar_inset")
        bar.set(padding_left=width)
        for button in own_buttons:
            button.set(visible=not window.get("native_controls"))

    window.on("titlebar_inset", lambda e: lay_out_bar(e.titlebar_inset))
    lay_out_bar()
    assert all(b.get("visible") is not MACOS for b in own_buttons)
    window.simulate("titlebar_inset", height=28, width=78)
    assert bar.get("padding_left") == 78.0
