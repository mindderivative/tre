#!/usr/bin/env python3
"""Custom cursors: a pointer shape drawn from pixels.

`tre.CursorImage(rgba, width, height, hotspot)` is a cursor you draw: here a
reticle (a ring with a dot, drawn in Python from distances, no imaging library) over a
canvas card, while the rest of the window keeps the arrow. The engine decodes no image
format; for a PNG, decode it with your imaging library and pass the RGBA bytes.

Headless-CI-safe: renders `max_frames=60` and returns quietly without a display.
Pass `--watch` to keep the window open and move the pointer over the card.
See docs/reference/node.md.
"""

import math
import sys

import tre
from tre import App, Window

SIZE = 32


def reticle() -> tre.CursorImage:
    """A ring and a centre dot, anti-aliased by distance, in the accent colour."""
    rgba = bytearray(SIZE * SIZE * 4)
    centre = (SIZE - 1) / 2
    for y in range(SIZE):
        for x in range(SIZE):
            d = math.hypot(x - centre, y - centre)
            ring = max(0.0, 1.0 - abs(d - 11.0) / 1.5)
            dot = max(0.0, 1.0 - d / 2.0)
            alpha = min(1.0, ring + dot)
            i = (y * SIZE + x) * 4
            rgba[i : i + 4] = bytes((0x67, 0x50, 0xA4, round(alpha * 255)))
    return tre.CursorImage(bytes(rgba), SIZE, SIZE, hotspot=(SIZE // 2, SIZE // 2))


window = Window(width=360, height=220, title="tre -- custom cursor")
card = window.create("box", width=300, height=140, x=30, y=40, position="absolute",
                     corner_radius=16, fill=(0xF3, 0xED, 0xF7, 0xFF))
card.add_child(window.create("text", text="Aim here", font_size=20, width=200, height=30,
                             x=20, y=20, position="absolute", fill=(0x1C, 0x1B, 0x1F, 0xFF)))
cursor = reticle()
card.set(cursor=cursor)
window.root.add_child(card)

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 60)
print("custom_cursor.py: cursor", cursor, "ready =", cursor.ready)
print("custom_cursor.py: exited cleanly")
