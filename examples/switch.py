#!/usr/bin/env python3
"""A switch built from primitives: two boxes, a few listeners, and
animation -- the widget `docs/guide/building-a-widget.md` walks through.

It toggles on a click, on Space or Enter while focused, and on an
assistive technology's activate request (which arrives as `click`); it
reports `checked` to assistive technology; its thumb eases across and its
track eases to the "on" color; a hover grows the thumb; and a focus ring
shows only when focus arrived by keyboard.

The script checks all of that headlessly with `window.simulate` and
`window.advance`, then opens the window for 120 frames. Headless-CI-safe:
`App.run()` returns quietly where no display or GPU is reachable.
"""

from tre import App, Event, Window

TRACK_OFF = (0xE6, 0xE0, 0xE9, 0xFF)
TRACK_ON = (0x67, 0x50, 0xA4, 0xFF)
THUMB = (0xFF, 0xFF, 0xFF, 0xFF)
RING = (0x67, 0x50, 0xA4, 0x80)
NO_RING = (0, 0, 0, 0)
EASE = (0.2, 0.0, 0.0, 1.0)  # a standard decelerating curve
TRAVEL = 20.0  # track width 52 - padding 2*4 - thumb 24


class Switch:
    """An on/off switch. `on_change(checked)` runs after each toggle."""

    def __init__(self, window, label, checked=False, on_change=None):
        self.checked = checked
        self.on_change = on_change
        # The track is the widget: it takes focus, carries the
        # accessibility role and state, and lays out the thumb.
        self.node = window.create(
            "box", width=52, height=32, corner_radius=16, padding=4,
            align_items="center", fill=TRACK_OFF,
            shadows=[(NO_RING, 0, 0, 0, 0)],
            role="switch", label=label, checked=checked,
            focusable=True, cursor="pointer",
        )
        self.thumb = window.create(
            "box", width=24, height=24, corner_radius=12, fill=THUMB,
            shadows=[((0, 0, 0, 60), 0, 1, 2, 0)],
        )
        self.node.add_child(self.thumb)

        # A click anywhere in the switch bubbles up to the track, and
        # Space, Enter, and assistive activation arrive as `click` too.
        self.node.on("click", self.toggle)
        self.node.on("pointer_enter", lambda: self.thumb.animate("scale", 1.1, 100))
        self.node.on("pointer_leave", lambda: self.thumb.animate("scale", 1.0, 100))
        self.node.on("focus", self.show_focus)
        self.node.on("unfocus", lambda: self.node.animate("shadows", [(NO_RING, 0, 0, 0, 0)], 100))
        self.paint(duration_ms=0)

    def toggle(self):
        self.checked = not self.checked
        self.node.set(checked=self.checked)
        self.paint(duration_ms=200)
        if self.on_change:
            self.on_change(self.checked)

    def paint(self, duration_ms):
        self.thumb.animate("translate_x", TRAVEL if self.checked else 0.0, duration_ms, easing=EASE)
        self.node.animate("fill", TRACK_ON if self.checked else TRACK_OFF, duration_ms, easing=EASE)

    def show_focus(self, event: Event):
        # A ring for keyboard focus only -- the `:focus-visible` rule.
        if event.focus_visible:
            self.node.animate("shadows", [(RING, 0, 0, 0, 3)], 100)


def main():
    window = Window(width=320, height=120, title="tre -- switch")
    window.root.set(align_items="center", gap=12)
    caption = window.create("text", text="Wi-Fi", font_size=16, width=60, height=20)
    changes = []
    wifi = Switch(window, "Wi-Fi", on_change=changes.append)
    window.root.add_child(caption)
    window.root.add_child(wifi.node)

    # -- headless checks ---------------------------------------------------------
    window.advance(0)  # pin the clock so animations run on `advance`

    window.simulate("click", node=wifi.thumb)  # a press on the thumb bubbles up
    window.advance(200)
    assert wifi.node.get("checked") is True and changes == [True]
    assert wifi.thumb.get("translate_x") == TRAVEL
    assert wifi.node.get("fill") == TRACK_ON
    assert wifi.node.get("focused"), "a click focuses the nearest focusable node"
    assert wifi.node.get("shadows")[0][0] == NO_RING, "no ring after a pointer press"

    window.simulate("key_down", key="space")  # Space activates the focused node
    window.advance(200)
    assert changes == [True, False] and wifi.thumb.get("translate_x") == 0.0

    window.simulate("unfocus", node=wifi.node)
    window.simulate("key_down", key="tab")  # back in by keyboard
    window.advance(100)
    assert wifi.node.get("shadows")[0][0] == RING, "keyboard focus shows the ring"

    window.simulate("pointer_move", node=wifi.node)
    window.advance(100)
    assert wifi.thumb.get("scale") == 1.1
    print(f"switch.py: toggled {len(changes)} times, checks passed")

    app = App()
    app.add_window(window)
    app.run(max_frames=120)
    print("switch.py: exited cleanly")


if __name__ == "__main__":
    main()
