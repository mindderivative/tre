#!/usr/bin/env python3
"""Gradients: linear, radial and sweep fills on boxes, and one that animates.

`fill=Gradient.linear(...)`, `Gradient.radial(...)` and `Gradient.sweep(...)`
give a box a ramp instead of a flat colour. The first card's gradient eases to
another of the same shape (angle and colours together); the radial card is a
soft light that fades to transparent; the sweep is a colour wheel.

Headless-CI-safe: renders `max_frames=120` and returns quietly without a
display. Pass `--watch` to keep the window open. See docs/reference/paint.md.
"""

import sys

from tre import App, Gradient, Window

PURPLE = (0x67, 0x50, 0xA4, 0xFF)
DEEP = (0x21, 0x00, 0x5D, 0xFF)
TEAL = (0x03, 0xDA, 0xC6, 0xFF)
NAVY = (0x01, 0x2A, 0x4A, 0xFF)

window = Window(width=620, height=220, title="tre -- gradients")
window.root.set(fill=(0x1C, 0x1B, 0x1F, 0xFF), flex_direction="horizontal", gap=16)

first = window.create(
    "box", width=170, height=170, corner_radius=20,
    fill=Gradient.linear([PURPLE, DEEP], angle=135),
)
glow = window.create(
    "box", width=170, height=170, corner_radius=20,
    fill=Gradient.radial([(255, 255, 255, 255), (255, 255, 255, 0)], radius=0.9),
)
wheel = window.create(
    "box", width=170, height=170, corner_radius=85,
    fill=Gradient.sweep(
        [(255, 0, 0, 255), (255, 255, 0, 255), (0, 255, 0, 255),
         (0, 255, 255, 255), (0, 0, 255, 255), (255, 0, 255, 255), (255, 0, 0, 255)]
    ),
)
for card in (first, glow, wheel):
    window.root.add_child(card)


def swing(forward: bool) -> None:
    """Ease the first card between two gradients of the same shape, forever."""
    angle, stops = (315, [TEAL, NAVY]) if forward else (135, [PURPLE, DEEP])
    first.animate(
        "fill", Gradient.linear(stops, angle=angle), 1500,
        on_complete=lambda: swing(not forward),
    )


swing(True)

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 120)
print("gradients.py: exited cleanly")
