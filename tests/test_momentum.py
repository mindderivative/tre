"""0.5.4 (#136): a flicked scroll view or virtual list coasts to a stop.

Driven by simulated touches and `window.advance` for time: the finger's
release velocity is real (the recognizer measures it from the moves and the
clock), so a fast flick coasts and a slow drag does not.
"""

import pytest

from tre import Window


def scroll_scene(height=200, content=3000):
    window = Window(width=200, height=height)
    window.root.set(padding=0)
    view = window.create("scroll_view", width=200, height=height)
    window.root.add_child(view)
    body = window.create("box", width=200, height=content)
    view.add_child(body)
    window.snapshot()
    return window, view, body


def flick(window, y0=150, y1=40, steps=5, ms=16):
    """A finger moving up from y0 to y1 in `steps` equal moves, `ms` apart, then lifting."""
    window.simulate("touch_start", x=100, y=y0)
    for i in range(1, steps + 1):
        window.advance(ms)
        window.simulate("touch_move", x=100, y=y0 + (y1 - y0) * i / steps)
    window.simulate("touch_end", x=100, y=y1)


def test_a_fast_flick_keeps_scrolling_after_the_finger_lifts():
    window, view, _ = scroll_scene()
    flick(window)
    at_lift = view.get("scroll_offset")
    assert at_lift > 0
    window.advance(100)
    assert view.get("scroll_offset") > at_lift + 20, "it coasts"
    window.advance(300)
    mid = view.get("scroll_offset")
    window.advance(3000)
    rest = view.get("scroll_offset")
    assert rest > mid, "still slowing"
    window.advance(1000)
    assert view.get("scroll_offset") == rest, "and then stopped"


def test_a_coast_slows_down():
    window, view, _ = scroll_scene()
    flick(window)
    samples = []
    for _ in range(4):
        before = view.get("scroll_offset")
        window.advance(80)
        samples.append(view.get("scroll_offset") - before)
    assert samples == sorted(samples, reverse=True), samples
    assert samples[0] > samples[-1]


def test_a_slow_drag_does_not_coast():
    window, view, _ = scroll_scene()
    flick(window, y0=150, y1=100, steps=5, ms=200)  # 50 px in a second
    at_lift = view.get("scroll_offset")
    window.advance(2000)
    assert view.get("scroll_offset") == at_lift


def test_a_finger_landing_catches_the_coast():
    window, view, _ = scroll_scene()
    flick(window)
    window.advance(100)
    caught = view.get("scroll_offset")
    window.simulate("touch_start", x=100, y=100)
    window.advance(500)
    assert view.get("scroll_offset") == caught
    window.simulate("touch_end", x=100, y=100)


def test_the_coast_stops_at_the_end_of_the_content():
    window, view, _ = scroll_scene(content=260)  # only 60 px to scroll
    flick(window, y0=190, y1=20, steps=4, ms=10)
    window.advance(3000)
    assert view.get("scroll_offset") == pytest.approx(60.0)


def test_the_app_taking_the_pan_means_no_coast():
    window, view, body = scroll_scene()
    body.on("pan", lambda e: None)
    flick(window)
    window.advance(2000)
    assert view.get("scroll_offset") == 0


def test_a_flick_toward_the_blocked_end_does_nothing():
    window, view, _ = scroll_scene()
    flick(window, y0=40, y1=150)  # finger moving down: toward the start, already there
    window.advance(2000)
    assert view.get("scroll_offset") == 0


def test_a_virtual_list_coasts_too():
    window = Window(width=200, height=200)
    window.root.set(padding=0)
    asked = []

    def row(index):
        asked.append(index)
        return window.create("box", width=200, height=28)

    lst = window.create("virtual_list", width=200, height=200, item_count=1000, item_extent=30,
                        materialize=row)
    window.root.add_child(lst)
    window.snapshot()
    flick(window)
    window.advance(16)
    at_lift = max(asked)
    window.advance(200)
    window.advance(16)
    assert max(asked) > at_lift + 2, "later rows were built as it coasted"
    window.advance(4000)
    rest = max(asked)
    window.advance(1000)
    assert max(asked) == rest, "and then it stopped"


def test_the_wheel_stops_a_coast_too():
    window, view, _ = scroll_scene()
    flick(window)
    window.advance(100)
    window.simulate("wheel", x=100, y=100, delta_y=0.0001)
    here = view.get("scroll_offset")
    window.advance(1000)
    assert view.get("scroll_offset") == pytest.approx(here, abs=0.5)
