#!/usr/bin/env python3
"""A slider built from primitives: a track box with a fill and a thumb
inside it. It shows the input and accessibility building blocks together:

- a press sets the value and captures the pointer, so a drag keeps
  tracking -- and clamps -- after the pointer leaves the track;
- the arrow keys step it while it's focused;
- it reports `role="slider"` and its value and range to assistive
  technology, and answers `increment`, `decrement`, and `set_value`
  requests through `a11y_action`.

The script checks each behavior with `window.simulate`, then opens the
window. Headless-CI-safe: `App.run()` renders `max_frames=60` and returns
quietly without a display or GPU. See docs/guide/accessibility.md.
"""

from tre import App, Event, Window

WIDTH, THUMB = 240.0, 20.0
STEP = 0.1
TRACK = (0xE6, 0xE0, 0xE9, 0xFF)
ACCENT = (0x67, 0x50, 0xA4, 0xFF)


class Slider:
    def __init__(self, window, label, value=0.5, on_change=None):
        self.value = value
        self.on_change = on_change
        self.dragging = False
        self.node = window.create(
            "box", width=WIDTH, height=THUMB, align_items="center",
            role="slider", label=label, value_min=0.0, value_max=1.0,
            value_step=STEP, focusable=True, cursor="pointer",
        )
        rail = window.create("box", width=WIDTH, height=4, corner_radius=2, fill=TRACK,
                             position="absolute", x=0, y=(THUMB - 4) / 2)
        self.fill = window.create("box", height=4, corner_radius=2, fill=ACCENT,
                                  position="absolute", x=0, y=(THUMB - 4) / 2)
        self.thumb = window.create("box", width=THUMB, height=THUMB, corner_radius=THUMB / 2,
                                   fill=ACCENT, position="absolute", y=0)
        for part in (rail, self.fill, self.thumb):
            self.node.add_child(part)

        self.node.on("pointer_down", self.press)
        self.node.on("pointer_move", self.drag)
        self.node.on("pointer_up", self.release)
        self.node.on("key_down", self.key)
        self.node.on("a11y_action", self.action)
        self.set_value(value)

    def set_value(self, value):
        self.value = round(min(1.0, max(0.0, value)), 6)
        x = self.value * (WIDTH - THUMB)
        self.thumb.set(x=x)
        self.fill.set(width=x + THUMB / 2)
        self.node.set(value=self.value)  # what a screen reader announces
        if self.on_change:
            self.on_change(self.value)

    def from_pointer(self, event: Event):
        self.set_value((event.x - THUMB / 2) / (WIDTH - THUMB))

    def press(self, event: Event):
        self.dragging = True
        self.node.capture_pointer()  # held until the button is released
        self.from_pointer(event)

    def drag(self, event: Event):
        if self.dragging:
            self.from_pointer(event)

    def release(self, event: Event):
        self.dragging = False

    def key(self, event: Event):
        if event.key in ("arrow_right", "arrow_up"):
            self.set_value(self.value + STEP)
        elif event.key in ("arrow_left", "arrow_down"):
            self.set_value(self.value - STEP)

    def action(self, event: Event):
        if event.action == "increment":
            self.set_value(self.value + STEP)
        elif event.action == "decrement":
            self.set_value(self.value - STEP)
        elif event.action == "set_value":
            self.set_value(float(event.value))


def main():
    window = Window(width=320, height=120, title="tre -- slider")
    window.root.set(align_items="center")
    volume = Slider(window, "Volume", value=0.5)
    window.root.add_child(volume.node)
    span = WIDTH - THUMB

    # -- checks ------------------------------------------------------------------
    window.simulate("pointer_down", node=volume.node, x=THUMB / 2 + span * 0.25, y=10)
    assert volume.value == 0.25 and volume.node.get("value") == 0.25
    window.simulate("pointer_move", x=310, y=110)  # far off the track: still tracking
    assert volume.value == 1.0, "clamped at the end"
    window.simulate("pointer_up", x=310, y=110)
    window.simulate("pointer_move", node=volume.node, x=THUMB / 2, y=10)
    assert volume.value == 1.0, "released: moving no longer drags"

    assert volume.node.get("focused"), "the press focused it"
    window.simulate("key_down", key="arrow_left")
    window.simulate("key_down", key="arrow_left")
    assert volume.value == 0.8

    window.simulate("a11y_action", node=volume.node, action="decrement")
    window.simulate("a11y_action", node=volume.node, action="set_value", value=0.3)
    assert volume.value == 0.3 and volume.thumb.get("x") == 0.3 * span
    print("slider.py: checks passed")

    app = App()
    app.add_window(window)
    app.run(max_frames=60)
    print("slider.py: exited cleanly")


if __name__ == "__main__":
    main()
