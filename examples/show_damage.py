#!/usr/bin/env python3
"""Show damage: see what partial redraw repaints each frame.

`window.set(show_damage=True)` tints each frame's redrawn areas magenta,
or outlines the window's edge in orange when a frame redraws everything.
Here one card pulses while the rest stay still, so only its box should
light up; resize or uncover the window and the whole frame redraws.

Once the window is open it prints `partial_redraw_active` -- whether
partial redraw is really in effect on this platform's surface.

Headless-CI-safe: renders `max_frames=120` and returns quietly without a
display. Pass `--watch` to keep the window open until you close it -- the
way to check partial redraw on a real desktop. See docs/api/python/window.md.
"""

import sys

from tre import App, Node, Window

PURPLE = (0x67, 0x50, 0xA4, 0xFF)
TEAL = (0x03, 0xDA, 0xC6, 0xFF)

window = Window(width=520, height=240, title="tre -- show_damage")
window.set(show_damage=True)
assert window.get("show_damage") is True

cards = [window.create("box", width=90, height=90, fill=PURPLE) for _ in range(4)]
for card in cards:
    window.root.add_child(card)


def pulse(card: Node, to: tuple[int, int, int, int], back: tuple[int, int, int, int]) -> None:
    """Animate `card`'s fill to `to` and back, forever."""
    card.animate("fill", to, 700, on_complete=lambda: pulse(card, back, to))


pulse(cards[1], TEAL, PURPLE)

app = App()
app.add_window(window)
app.thread_handle().call_soon(
    lambda: print("show_damage.py: partial_redraw_active =", window.get("partial_redraw_active"))
)
app.run(max_frames=None if "--watch" in sys.argv else 120)
print("show_damage.py: exited cleanly")
