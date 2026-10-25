"""0.5.8 (#132): rotation and swipe gestures, the touch slop, and the touch-target hit slop.

No touch hardware needed: `window.simulate("touch_start", ...)` delivers the same
`InputEvent::Touch` the platform does, and `window.advance` moves the clock. The
recognizer's own arithmetic is tested in `engine-core/src/gesture.rs`.
"""

import math

import pytest

from tre import Window


def scene(**props):
    window = Window(width=400, height=300)
    window.root.set(padding=0)
    box = window.create("box", width=200, height=200, x=20, y=20, position="absolute",
                        fill=(200, 200, 200, 255), **props)
    window.root.add_child(box)
    return window, box


def touch(window, phase, x, y, id=0):
    window.simulate(f"touch_{phase}", x=x, y=y, id=id)


def pair(radians, r=100):
    dx, dy = math.cos(radians) * r / 2, math.sin(radians) * r / 2
    return (120 - dx, 120 - dy), (120 + dx, 120 + dy)


def listen(node, *names):
    log = []
    for name in names:
        node.on(name, lambda e, name=name: log.append((name, e)))
    return log


def test_two_fingers_turning_are_a_rotate_event_with_the_angle_in_radians():
    window, box = scene()
    log = listen(box, "rotate", "pinch")
    a, b = pair(0)
    touch(window, "start", *a, id=1)
    touch(window, "start", *b, id=2)
    for radians in (0.05, 0.3, 0.6):
        a, b = pair(radians)
        touch(window, "move", *a, id=1)
        touch(window, "move", *b, id=2)
    touch(window, "end", *b, id=2)
    turns = [e for n, e in log if n == "rotate"]
    assert turns[0].phase == "began" and turns[-1].phase == "ended"
    changed = [e for e in turns if e.phase in ("began", "changed")]
    assert changed[-1].rotation == pytest.approx(0.6)
    assert sum(e.rotation_delta for e in changed) == pytest.approx(0.6)
    assert turns[-1].rotation == pytest.approx(0.6)
    # A rotation comes with the pinch the same fingers make.
    assert any(n == "pinch" for n, _ in log)


def test_fingers_that_only_pinch_send_no_rotate_event():
    window, box = scene()
    log = listen(box, "rotate", "pinch")
    touch(window, "start", 100, 120, id=1)
    touch(window, "start", 200, 120, id=2)
    touch(window, "move", 260, 120, id=2)
    touch(window, "end", 260, 120, id=2)
    assert [n for n, _ in log if n == "rotate"] == []
    assert any(n == "pinch" for n, _ in log)


def flick(window, x0, y0, x1, y1, ms=150, steps=5):
    touch(window, "start", x0, y0)
    for i in range(1, steps + 1):
        window.advance(ms / steps)
        touch(window, "move", x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps)
    touch(window, "end", x1, y1)


@pytest.mark.parametrize("end,direction", [((320, 125), "right"), ((-80, 115), "left"),
                                           ((125, 280), "down"), ((115, -80), "up")])
def test_a_fast_long_pan_is_a_swipe_in_its_direction(end, direction):
    window, box = scene()
    log = listen(box, "pan", "swipe")
    flick(window, 120, 120, *end)
    swipes = [e for n, e in log if n == "swipe"]
    assert len(swipes) == 1
    assert swipes[0].direction == direction
    assert math.hypot(swipes[0].velocity_x, swipes[0].velocity_y) >= 600
    # After the pan's own end.
    assert [n for n, _ in log][-2:] == ["pan", "swipe"]


def test_a_slow_pan_is_not_a_swipe():
    window, box = scene()
    log = listen(box, "swipe")
    flick(window, 100, 120, 200, 120, ms=2000)
    assert log == []


def test_the_touch_slop_is_settable_and_readable():
    window, box = scene()
    assert window.get("touch_slop") == 10.0
    window.set(touch_slop=30)
    assert window.get("touch_slop") == 30.0
    log = listen(box, "pan", "tap")
    touch(window, "start", 100, 120)
    touch(window, "move", 120, 120)       # 20 px: inside the 30 px slop
    touch(window, "end", 120, 120)
    assert [n for n, _ in log] == ["tap"]
    with pytest.raises(ValueError, match="touch_slop"):
        window.set(touch_slop=-1)
    with pytest.raises(ValueError, match="touch_slop"):
        window.set(touch_slop="wide")


def small_target_scene():
    window = Window(width=400, height=300)
    window.root.set(padding=0)
    button = window.create("box", width=40, height=40, x=100, y=100, position="absolute",
                           fill=(100, 100, 220, 255))
    other = window.create("box", width=40, height=40, x=250, y=100, position="absolute")
    window.root.add_child(button)
    window.root.add_child(other)
    clicks = []
    button.on("click", lambda e: clicks.append("button"))
    other.on("click", lambda e: clicks.append("other"))
    return window, button, clicks


def test_by_default_a_touch_just_outside_a_button_hits_nothing():
    window, button, clicks = small_target_scene()
    assert window.get("touch_target_slop") == 0.0
    touch(window, "start", 92, 120)   # 8 px left of the button
    touch(window, "end", 92, 120)
    assert clicks == []


def test_with_a_touch_target_slop_a_touch_near_a_button_presses_it():
    window, button, clicks = small_target_scene()
    window.set(touch_target_slop=12)
    assert window.get("touch_target_slop") == 12.0
    touch(window, "start", 92, 120)   # 8 px left of the button, inside the 12 px slop
    touch(window, "end", 92, 120)
    assert clicks == ["button"]
    # Past the slop is still nothing.
    touch(window, "start", 80, 120)   # 20 px away
    touch(window, "end", 80, 120)
    assert clicks == ["button"]


def test_a_touch_on_a_button_is_not_pulled_to_a_nearer_one():
    window, button, clicks = small_target_scene()
    window.set(touch_target_slop=200)
    touch(window, "start", 260, 120)  # on `other`, much nearer than `button`'s box is
    touch(window, "end", 260, 120)
    assert clicks == ["other"]


def test_the_nearest_of_two_listening_buttons_wins():
    window, button, clicks = small_target_scene()
    window.set(touch_target_slop=40)
    touch(window, "start", 230, 120)  # 20 px from `other`, 90 px from `button`
    touch(window, "end", 230, 120)
    assert clicks == ["other"]


def test_a_finger_that_drifts_within_the_slop_still_clicks():
    window, button, clicks = small_target_scene()
    window.set(touch_target_slop=20)
    touch(window, "start", 92, 120)
    touch(window, "move", 88, 126)
    touch(window, "end", 90, 124)
    assert clicks == ["button"]


def test_the_touch_event_itself_goes_to_the_node_the_touch_was_given_to():
    window, button, clicks = small_target_scene()
    window.set(touch_target_slop=12)
    seen = listen(button, "touch_start", "tap")
    touch(window, "start", 92, 120)
    touch(window, "end", 92, 120)
    assert [n for n, _ in seen] == ["touch_start", "tap"]
