#!/usr/bin/env python3
"""Effects: blur, frosted glass (backdrop_blur) and blend modes.

A bar of colour runs behind three cards. The first is blurred (`blur`: the card
and what it holds go soft), the second is frosted glass (`backdrop_blur` with a
translucent white fill: the bar behind it is blurred, inside the card only), the
third multiplies with what is behind it (`blend_mode="multiply"`). The frosted
card's blur eases in and out.

Headless-CI-safe: renders `max_frames=120` and returns quietly without a
display. Pass `--watch` to keep the window open. See docs/reference/paint.md.
"""

import sys

from tre import App, Window

RED = (0xE6, 0x39, 0x46, 0xFF)
TEAL = (0x2A, 0x9D, 0x8F, 0xFF)
SAND = (0xE9, 0xC4, 0x6A, 0xFF)
INK = (0x1C, 0x1B, 0x1F, 0xFF)

window = Window(width=640, height=240, title="tre -- effects")
window.root.set(fill=INK, padding=0)

# Stripes to blur and blend against.
for i, color in enumerate((RED, TEAL, SAND, RED, TEAL, SAND, RED, TEAL)):
    stripe = window.create(
        "box", width=80, height=240, x=i * 80, y=0, position="absolute", fill=color
    )
    window.root.add_child(stripe)


def card(x: int, **props) -> object:
    node = window.create(
        "box", width=170, height=170, x=x, y=35, position="absolute",
        corner_radius=20, **props,
    )
    window.root.add_child(node)
    return node


blurred = card(20, fill=(255, 255, 255, 255), blur=3)
frosted = card(235, fill=(255, 255, 255, 70), backdrop_blur=0)
multiplied = card(450, fill=(255, 150, 60, 255), blend_mode="multiply")


def pulse(up: bool) -> None:
    frosted.animate("backdrop_blur", 14 if up else 2, 1500, on_complete=lambda: pulse(not up))


pulse(True)

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 120)
print("effects.py: exited cleanly")
