"""0.5.4 (#113): touch events, gestures, and the finger standing in for the pointer.

No touch hardware needed: `window.simulate("touch_start", ...)` and friends
deliver the same `InputEvent::Touch` the platform does, and `window.advance`
moves the clock for long presses.
"""

import pytest

from tre import Window


def scene():
    window = Window(width=300, height=300)
    window.root.set(padding=0)
    box = window.create("box", width=200, height=200, x=20, y=20, position="absolute",
                        fill=(200, 200, 200, 255))
    window.root.add_child(box)
    log = []
    for name in ("touch_start", "touch_move", "touch_end", "touch_cancel", "tap",
                 "long_press", "pan", "pinch", "click", "pointer_down", "pointer_up",
                 "pointer_cancel", "pointer_enter", "pointer_leave"):
        box.on(name, lambda e, name=name: log.append((name, e)))
    return window, box, log


def names(log):
    return [n for n, _ in log]


def touch(window, phase, x, y, id=0, node=None):
    window.simulate(f"touch_{phase}", node=node, x=x, y=y, id=id)


def test_a_tap_is_touch_events_a_tap_and_a_click():
    window, box, log = scene()
    touch(window, "start", 100, 100)
    touch(window, "end", 100, 100)
    seen = names(log)
    for name in ("touch_start", "touch_end", "tap", "pointer_down", "pointer_up", "click"):
        assert name in seen, (name, seen)
    # Each touch event comes first, then the pointer events it stands in for,
    # then the gesture it completes.
    assert seen.index("touch_start") < seen.index("pointer_down")
    assert seen.index("touch_end") < seen.index("pointer_up") < seen.index("click") < seen.index("tap")
    assert "pan" not in seen and "long_press" not in seen
    start = next(e for n, e in log if n == "touch_start")
    assert start.pointer_id == 0
    assert (start.window_x, start.window_y) == (100.0, 100.0)
    tap = next(e for n, e in log if n == "tap")
    assert tap.count == 1


def test_two_taps_close_together_are_a_double_tap():
    window, box, log = scene()
    for _ in range(2):
        touch(window, "start", 100, 100)
        touch(window, "end", 100, 100)
        window.advance(100)
    counts = [e.count for n, e in log if n == "tap"]
    assert counts == [1, 2]


def test_a_held_touch_is_a_long_press_and_then_no_tap():
    window, box, log = scene()
    touch(window, "start", 100, 100)
    window.advance(300)
    assert "long_press" not in names(log)
    window.advance(250)
    assert names(log).count("long_press") == 1
    touch(window, "end", 100, 100)
    assert "tap" not in names(log)
    event = next(e for n, e in log if n == "long_press")
    assert (event.window_x, event.window_y) == (100.0, 100.0)


def test_a_drag_is_a_pan_with_steps_and_totals_and_cancels_the_click():
    window, box, log = scene()
    touch(window, "start", 100, 100)
    touch(window, "move", 105, 100)
    assert "pan" not in names(log), "inside the slop"
    touch(window, "move", 125, 110)
    touch(window, "move", 145, 130)
    touch(window, "end", 145, 130)
    pans = [e for n, e in log if n == "pan"]
    assert [p.phase for p in pans] == ["began", "changed", "ended"]
    assert (pans[0].delta_x, pans[0].delta_y) == (25.0, 10.0)
    assert (pans[1].delta_x, pans[1].delta_y) == (20.0, 20.0)
    assert (pans[2].total_x, pans[2].total_y) == (45.0, 30.0)
    assert "click" not in names(log), "a drag is not a click"
    assert "pointer_cancel" in names(log)
    assert "tap" not in names(log)


def test_a_pan_ends_with_a_velocity():
    window, box, log = scene()
    touch(window, "start", 50, 100)
    for i in range(1, 6):
        window.advance(20)
        touch(window, "move", 50 + i * 20, 100)
    touch(window, "end", 150, 100)
    ended = [e for n, e in log if n == "pan"][-1]
    assert ended.phase == "ended"
    assert ended.velocity_x == pytest.approx(1000, rel=0.3)


def test_two_fingers_pinch():
    window, box, log = scene()
    touch(window, "start", 100, 100, id=1)
    touch(window, "start", 200, 100, id=2)
    touch(window, "move", 300, 100, id=2)
    touch(window, "move", 250, 100, id=2)
    touch(window, "end", 250, 100, id=2)
    pinches = [e for n, e in log if n == "pinch"]
    assert [p.phase for p in pinches] == ["began", "changed", "ended"]
    assert pinches[0].scale == pytest.approx(2.0)
    assert pinches[1].scale == pytest.approx(1.5)
    assert pinches[1].scale_delta == pytest.approx(0.75)
    assert (pinches[0].window_x, pinches[0].window_y) == (200.0, 100.0)
    # Only the first finger was ever a pointer.
    touch(window, "end", 100, 100, id=1)
    assert names(log).count("touch_start") == 2


def test_a_second_finger_is_not_a_second_pointer():
    window, box, log = scene()
    touch(window, "start", 100, 100, id=1)
    touch(window, "start", 150, 100, id=2)
    assert names(log).count("pointer_down") == 1


def test_a_touch_stays_with_the_node_it_landed_on():
    window = Window(width=300, height=300)
    window.root.set(padding=0)
    a = window.create("box", width=100, height=100, x=0, y=0, position="absolute")
    b = window.create("box", width=100, height=100, x=150, y=0, position="absolute")
    for node in (a, b):
        window.root.add_child(node)
    seen = {"a": [], "b": []}
    a.on("touch_move", lambda e: seen["a"].append(e.window_x))
    b.on("touch_move", lambda e: seen["b"].append(e.window_x))
    touch(window, "start", 50, 50)
    touch(window, "move", 200, 50)   # over b now
    assert seen == {"a": [200.0], "b": []}


def test_a_cancelled_touch_cancels_its_pan_and_press():
    window, box, log = scene()
    touch(window, "start", 100, 100)
    touch(window, "move", 140, 100)
    touch(window, "cancel", 140, 100)
    assert "touch_cancel" in names(log)
    pans = [e.phase for n, e in log if n == "pan"]
    assert pans == ["began", "cancelled"]
    assert "click" not in names(log)


def test_a_pan_scrolls_a_scroll_view_unless_something_listens_for_pan():
    def build(listen):
        window = Window(width=200, height=200)
        window.root.set(padding=0)
        view = window.create("scroll_view", width=200, height=200)
        window.root.add_child(view)
        content = window.create("box", width=200, height=1000)
        view.add_child(content)
        if listen:
            content.on("pan", lambda e: None)
        window.snapshot()
        return window, view

    window, view = build(listen=False)
    touch(window, "start", 100, 150, node=None)
    touch(window, "move", 100, 120)
    touch(window, "move", 100, 80)
    touch(window, "end", 100, 80)
    assert view.get("scroll_offset") > 40, "dragging up scrolls the content up"

    window, view = build(listen=True)
    touch(window, "start", 100, 150)
    touch(window, "move", 100, 80)
    touch(window, "end", 100, 80)
    assert view.get("scroll_offset") == 0, "the app took the pan"


def test_trackpad_pinch_is_a_pinch_event_with_a_running_scale():
    window, box, log = scene()
    window.simulate("trackpad_pinch", node=box, delta=0.0, phase="started")
    window.simulate("trackpad_pinch", node=box, delta=0.5, phase="moved")
    window.simulate("trackpad_pinch", node=box, delta=1.0, phase="moved")
    window.simulate("trackpad_pinch", node=box, phase="ended")
    pinches = [e for n, e in log if n == "pinch"]
    assert [p.phase for p in pinches] == ["began", "changed", "changed", "ended"]
    assert [round(p.scale, 3) for p in pinches] == [1.0, 1.5, 3.0, 3.0]
    assert pinches[1].scale_delta == pytest.approx(1.5)


def test_touch_events_bubble_to_ancestors():
    window = Window(width=300, height=300)
    window.root.set(padding=0)
    outer = window.create("box", width=200, height=200, x=0, y=0, position="absolute")
    inner = window.create("box", width=100, height=100, x=10, y=10, position="absolute")
    window.root.add_child(outer)
    outer.add_child(inner)
    heard = []
    outer.on("tap", lambda e: heard.append(e.target == inner))
    touch(window, "start", 50, 50)
    touch(window, "end", 50, 50)
    assert heard == [True]


def test_bad_fields_are_errors():
    window, box, _ = scene()
    with pytest.raises(ValueError, match="id"):
        window.simulate("touch_start", x=1, y=1, id=-1)
    with pytest.raises(ValueError, match="phase"):
        window.simulate("trackpad_pinch", x=1, y=1, phase="sideways")
    with pytest.raises(ValueError, match="touch_start"):
        window.simulate("nonsense")
