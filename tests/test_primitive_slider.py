"""M94 Phase 3: the proof widget -- a slider built only from a box and the
M94 building blocks (listeners, pointer capture, keys, accessibility
properties, `a11y_action`), with no built-in slider behind it. It has to
do what the built-in one does: set its value from a press, follow a drag
even off the track, clamp, step with the arrow keys, and expose its value
and range to assistive technology.
"""

from __future__ import annotations

import tre

TRACK_WIDTH = 200.0
STEP = 0.1


class PrimitiveSlider:
    def __init__(self, window: tre.Window) -> None:
        self.track = window.add_rect((200, 200, 200, 255), TRACK_WIDTH, 20)
        self.value = 0.0
        self.dragging = False
        self.committed: list[float] = []
        self.track.set(
            role="slider",
            label="Volume",
            value=0.0,
            value_min=0.0,
            value_max=1.0,
            value_step=STEP,
            focusable=True,
            cursor="pointer",
        )
        self.track.on("pointer_down", self.press)
        self.track.on("pointer_move", self.drag)
        self.track.on("pointer_up", self.release)
        self.track.on("key_down", self.key)
        self.track.on("a11y_action", self.action)

    def set_value(self, value: float) -> None:
        self.value = round(min(1.0, max(0.0, value)), 6)
        self.track.set(value=self.value)

    def press(self, e: tre.Event) -> None:
        assert e.x is not None
        self.track.capture_pointer()
        self.dragging = True
        self.set_value(e.x / TRACK_WIDTH)

    def drag(self, e: tre.Event) -> None:
        if self.dragging:
            assert e.x is not None
            self.set_value(e.x / TRACK_WIDTH)

    def release(self, e: tre.Event) -> None:
        self.dragging = False
        self.committed.append(self.value)

    def key(self, e: tre.Event) -> None:
        if e.key == "arrow_right":
            self.set_value(self.value + STEP)
        elif e.key == "arrow_left":
            self.set_value(self.value - STEP)

    def action(self, e: tre.Event) -> None:
        if e.action == "increment":
            self.set_value(self.value + STEP)
        elif e.action == "decrement":
            self.set_value(self.value - STEP)
        elif e.action == "set_value" and e.value is not None:
            self.set_value(float(e.value))


def make() -> tuple[tre.Window, PrimitiveSlider]:
    w = tre.Window(400, 300, "primitive slider")
    return w, PrimitiveSlider(w)


def test_a_press_sets_the_value_from_its_position() -> None:
    w, slider = make()
    w.simulate("pointer_down", node=slider.track, x=50, y=10)
    assert slider.value == 0.25
    assert slider.track.get("value") == 0.25


def test_a_drag_keeps_tracking_off_the_track_and_clamps() -> None:
    w, slider = make()
    w.simulate("pointer_down", node=slider.track, x=50, y=10)
    # Far outside the track: capture keeps the events coming, and the
    # value clamps to the end.
    w.simulate("pointer_move", x=390, y=290)
    assert slider.value == 1.0
    w.simulate("pointer_move", node=slider.track, x=-30, y=10)
    assert slider.value == 0.0
    w.simulate("pointer_up", x=390, y=290)
    assert slider.committed == [0.0]
    # Released: moving over the track no longer changes the value.
    w.simulate("pointer_move", node=slider.track, x=150, y=10)
    assert slider.value == 0.0


def test_clicking_focuses_it_and_arrow_keys_step_it() -> None:
    w, slider = make()
    w.simulate("click", node=slider.track, x=100, y=10)
    assert slider.track.get("focused") is True
    w.simulate("key_down", key="arrow_right")
    assert slider.value == 0.6
    w.simulate("key_down", key="arrow_left")
    w.simulate("key_down", key="arrow_left")
    assert slider.value == 0.4


def test_assistive_technology_can_adjust_it() -> None:
    w, slider = make()
    w.simulate("a11y_action", node=slider.track, action="increment")
    assert slider.value == 0.1
    w.simulate("a11y_action", node=slider.track, action="set_value", value=0.75)
    assert slider.value == 0.75
    w.simulate("a11y_action", node=slider.track, action="decrement")
    assert slider.value == 0.65
    assert (
        slider.track.get("role"),
        slider.track.get("value_min"),
        slider.track.get("value_max"),
    ) == ("slider", 0.0, 1.0)
